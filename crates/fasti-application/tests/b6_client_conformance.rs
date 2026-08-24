#![cfg(feature = "conformance-fixture")]

//! B6 neutral source conformance.
//!
//! The public acceptance contract carries an opaque evidence digest, not source
//! structure, so it is source-neutral by construction. Pushing different
//! payloads through it therefore proves nothing: every payload takes one code
//! path. What can actually diverge between client shapes is *behavior* -- how a
//! client derives operation identity, what it claims about time, and what it
//! does when it retries.
//!
//! These tests model four archetypes with materially different operational
//! patterns and assert they share one outcome table. A vendor-specific branch
//! anywhere in the acceptance path would make exactly one archetype diverge.
//!
//! ```text
//!   archetype driver (builds the command the way that client would)
//!         |
//!         v
//!   B1ConformanceFixture (in-memory, no durability claim)
//!         |
//!         +--> first submit             -> Committed
//!         +--> resubmit same digest     -> Replayed, equal receipt value
//!         +--> resubmit changed digest  -> IdempotencyConflict, no mutation
//!         +--> foreign access           -> denied
//! ```
//!
//! Deliberately not covered: outbox draining. No outbox exists in `crates/`.
//! Offline behavior is covered only as far as the contract reaches -- a delayed
//! or reordered resubmission still deduplicates.

use fasti_application::{
    conformance::B1ConformanceFixture, AcceptObservationCommand, AcceptObservationOutcome,
    ProblemCode, RequestAccessContext,
};
use fasti_domain::{
    ClaimedTrust, EvidenceId, EvidenceReference, ObservedAt, OccurredAt, OperationId,
    RequestCorrelationId, Sha256Digest,
};

/// One client shape, described by how it builds a canonical command.
struct Archetype {
    name: &'static str,
    /// What the client can honestly claim about the time it reports.
    trust: ClaimedTrust,
    observed_at: &'static str,
    /// A backfill knows when the thing happened; a live heartbeat may not.
    occurred_at: Option<&'static str>,
}

/// Four shapes chosen because their operational patterns differ, not because
/// their payloads do.
const ARCHETYPES: &[Archetype] = &[
    // Historical bulk backfill. Times are the source's claim, not observation.
    Archetype {
        name: "batch_importer",
        trust: ClaimedTrust::SourceClaim,
        observed_at: "2026-08-22T10:11:12Z",
        occurred_at: Some("2019-04-02T20:00:00Z"),
    },
    // High-frequency player heartbeat observed on the device itself.
    Archetype {
        name: "live_player",
        trust: ClaimedTrust::DeviceObserved,
        observed_at: "2026-08-22T10:11:13Z",
        occurred_at: Some("2026-08-22T10:05:00Z"),
    },
    // Ephemeral browser extension. Unstable clock, no durable device identity.
    Archetype {
        name: "browser_extension",
        trust: ClaimedTrust::DeviceObserved,
        observed_at: "2026-08-22T10:11:14Z",
        occurred_at: None,
    },
    // Periodic diff pull. The service infers when consumption happened.
    Archetype {
        name: "polling_sync",
        trust: ClaimedTrust::Inferred,
        observed_at: "2026-08-22T10:11:15Z",
        occurred_at: Some("2026-08-22T09:00:00Z"),
    },
];

fn enroll(fixture: &B1ConformanceFixture) -> fasti_application::conformance::FixtureEnrollment {
    let initialization = fixture
        .initialize_node(RequestCorrelationId::new_v7())
        .expect("fresh fixture initializes")
        .into_inner();
    fixture
        .enroll_first_client(RequestCorrelationId::new_v7(), &initialization)
        .expect("first client enrolls")
        .into_inner()
}

/// Derive an operation id from a stable source key.
///
/// This is the part of source-neutral conformance that actually matters. An
/// importer that mints a random id per attempt duplicates its whole backfill
/// after a crash; one that derives the id from the source row survives a
/// restart because the server recognizes the replay. Fasti cannot detect the
/// difference from a single request, so the client contract has to be tested.
///
/// FNV-1a expanded into the UUIDv7 shape the id registry accepts. Not a
/// cryptographic hash and not trying to be -- it only has to be deterministic.
fn derive_operation_id(source_key: &str) -> OperationId {
    let mut state: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in source_key.as_bytes() {
        state ^= u64::from(*byte);
        state = state.wrapping_mul(0x0000_0100_0000_01b3);
    }

    let mut hex = String::with_capacity(64);
    let mut lane = state;
    for _ in 0..4 {
        hex.push_str(&format!("{lane:016x}"));
        lane = lane.wrapping_mul(0x9e37_79b9_7f4a_7c15).rotate_left(31);
    }
    hex.truncate(32);

    let mut nibbles: Vec<char> = hex.chars().collect();
    // Version nibble must be 7 or `from_uuid` rejects it; variant nibble must
    // be 8..b to satisfy the published operation-id pattern.
    nibbles[12] = '7';
    nibbles[16] = '8';
    let hex: String = nibbles.into_iter().collect();

    format!("op_{hex}")
        .parse()
        .expect("derived operation id is a valid v7 identifier")
}

fn command(
    archetype: &Archetype,
    access: RequestAccessContext,
    operation_id: OperationId,
    digest_byte: &str,
) -> AcceptObservationCommand {
    AcceptObservationCommand::new(
        RequestCorrelationId::new_v7(),
        access,
        operation_id,
        archetype
            .occurred_at
            .map(|value| OccurredAt::parse(value, archetype.trust).expect("valid occurred time")),
        ObservedAt::parse(archetype.observed_at, archetype.trust).expect("valid observed time"),
        // A fresh evidence id each attempt: a retrying client has no reason to
        // remember one. The semantic digest must not depend on it.
        EvidenceReference::new(
            EvidenceId::new_v7(),
            Sha256Digest::parse(format!("sha256:{}", digest_byte.repeat(32)))
                .expect("canonical digest"),
            32,
        ),
    )
}

// ---------------------------------------------------------------------------
// Parity: every archetype shares one outcome table
// ---------------------------------------------------------------------------

#[test]
fn every_archetype_shares_one_outcome_table() {
    for archetype in ARCHETYPES {
        let fixture = B1ConformanceFixture::new();
        let enrollment = enroll(&fixture);
        let access = *enrollment.access();
        let operation_id = derive_operation_id(archetype.name);

        let committed = fixture
            .accept_fixture(
                enrollment.credential_secret(),
                command(archetype, access, operation_id, "11"),
            )
            .unwrap_or_else(|_| panic!("{} first submit commits", archetype.name))
            .into_inner();
        assert!(
            matches!(committed, AcceptObservationOutcome::Committed(_)),
            "{} must commit on first submit",
            archetype.name
        );
        let original = committed.receipt().clone();

        let replayed = fixture
            .accept_fixture(
                enrollment.credential_secret(),
                command(archetype, access, operation_id, "11"),
            )
            .unwrap_or_else(|_| panic!("{} same digest replays", archetype.name))
            .into_inner();
        assert!(
            matches!(replayed, AcceptObservationOutcome::Replayed(_)),
            "{} retry must replay, not commit again",
            archetype.name
        );
        // Semantic equality of the in-memory receipt. This is `PartialEq` on
        // the application type, not a comparison of serialized transport
        // output, so it does not prove byte-identical API responses. Proving
        // that needs an API-level test that serializes AcceptObservationResponse.
        assert_eq!(
            replayed.receipt(),
            &original,
            "{} replay must return the original receipt value",
            archetype.name
        );

        let conflict = fixture
            .accept_fixture(
                enrollment.credential_secret(),
                command(archetype, access, operation_id, "22"),
            )
            .expect_err("changed digest must conflict");
        assert_eq!(
            conflict.code(),
            ProblemCode::IdempotencyConflict,
            "{} changed digest must conflict",
            archetype.name
        );

        // One operation, one receipt: the conflict mutated nothing.
        let state = fixture.inspect_fixture();
        assert_eq!(state.as_ref().operation_count, 1, "{}", archetype.name);
        assert_eq!(state.as_ref().receipt_count, 1, "{}", archetype.name);
    }
}

#[test]
fn every_archetype_denies_foreign_access_identically() {
    for archetype in ARCHETYPES {
        let fixture = B1ConformanceFixture::new();
        let enrollment = enroll(&fixture);

        // A separate node's enrollment. Structurally valid, wrong workspace.
        let other = B1ConformanceFixture::new();
        let foreign_access = *enroll(&other).access();

        let denied = fixture.accept_fixture(
            enrollment.credential_secret(),
            command(
                archetype,
                foreign_access,
                derive_operation_id(archetype.name),
                "33",
            ),
        );
        // `is_err()` alone would pass on an unrelated validation or internal
        // failure, so the authorization outcome is asserted exactly.
        let problem = denied.expect_err("foreign access must be denied");
        assert_eq!(
            problem.code(),
            ProblemCode::Forbidden,
            "{} must be denied as Forbidden, not some other failure",
            archetype.name
        );
        let state = fixture.inspect_fixture();
        assert_eq!(
            state.as_ref().operation_count,
            0,
            "{} denial must not record an operation",
            archetype.name
        );
        assert_eq!(
            state.as_ref().receipt_count,
            0,
            "{} denial must not record a receipt",
            archetype.name
        );
    }
}

// ---------------------------------------------------------------------------
// Archetype-specific operational patterns
// ---------------------------------------------------------------------------

#[test]
fn derived_operation_ids_are_stable_for_the_same_source_key() {
    // Determinism is the whole property. If this ever stops holding, the
    // restart test below would still pass while proving nothing.
    for key in ["floppy:row:42", "simkl:history:9001", ""] {
        assert_eq!(
            derive_operation_id(key),
            derive_operation_id(key),
            "derivation must be stable for {key:?}"
        );
    }
    assert_ne!(
        derive_operation_id("floppy:row:42"),
        derive_operation_id("floppy:row:43"),
        "distinct source rows must not collide"
    );
}

#[test]
fn batch_importer_survives_a_restart_without_duplicating_its_backfill() {
    let archetype = &ARCHETYPES[0];
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    let source_rows = ["floppy:row:1", "floppy:row:2", "floppy:row:3"];

    for row in source_rows {
        fixture
            .accept_fixture(
                enrollment.credential_secret(),
                command(archetype, access, derive_operation_id(row), "44"),
            )
            .expect("first pass commits")
            .into_inner();
    }
    assert_eq!(fixture.inspect_fixture().as_ref().operation_count, 3);

    // The importer process dies and starts again. It has kept no state, so it
    // re-derives every operation id from the source rows and resubmits.
    for row in source_rows {
        let outcome = fixture
            .accept_fixture(
                enrollment.credential_secret(),
                command(archetype, access, derive_operation_id(row), "44"),
            )
            .expect("second pass is accepted")
            .into_inner();
        assert!(
            matches!(outcome, AcceptObservationOutcome::Replayed(_)),
            "restarted import of {row} must replay, not duplicate"
        );
    }

    assert_eq!(
        fixture.inspect_fixture().as_ref().operation_count,
        3,
        "a restarted backfill must not grow the operation set"
    );
    assert_eq!(fixture.inspect_fixture().as_ref().receipt_count, 3);
}

#[test]
fn live_player_reconnect_burst_creates_no_duplicates() {
    let archetype = &ARCHETYPES[1];
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    // Heartbeats buffered while the link was down.
    let buffered: Vec<OperationId> = (0..5)
        .map(|beat| derive_operation_id(&format!("nuvio:session:7:beat:{beat}")))
        .collect();

    for operation_id in &buffered {
        fixture
            .accept_fixture(
                enrollment.credential_secret(),
                command(archetype, access, *operation_id, "55"),
            )
            .expect("buffered heartbeat is accepted");
    }
    assert_eq!(fixture.inspect_fixture().as_ref().operation_count, 5);

    // The client never saw the receipts, so on reconnect it drains the buffer
    // again -- and out of order, because ordering is not something an offline
    // queue can promise.
    for operation_id in buffered.iter().rev() {
        let outcome = fixture
            .accept_fixture(
                enrollment.credential_secret(),
                command(archetype, access, *operation_id, "55"),
            )
            .expect("redelivered heartbeat is accepted")
            .into_inner();
        assert!(
            matches!(outcome, AcceptObservationOutcome::Replayed(_)),
            "redelivery must replay"
        );
    }

    assert_eq!(
        fixture.inspect_fixture().as_ref().operation_count,
        5,
        "an out-of-order redelivery must not create duplicates"
    );
}

#[test]
fn browser_extension_cross_tab_duplicates_resolve_to_one_operation() {
    let archetype = &ARCHETYPES[2];
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    // Two tabs observe the same play and both submit it.
    let operation_id = derive_operation_id("webscrobbler:play:abc123");

    let first = fixture
        .accept_fixture(
            enrollment.credential_secret(),
            command(archetype, access, operation_id, "66"),
        )
        .expect("first tab commits")
        .into_inner();
    let second = fixture
        .accept_fixture(
            enrollment.credential_secret(),
            command(archetype, access, operation_id, "66"),
        )
        .expect("second tab is accepted")
        .into_inner();

    assert!(matches!(first, AcceptObservationOutcome::Committed(_)));
    assert!(matches!(second, AcceptObservationOutcome::Replayed(_)));
    assert_eq!(first.receipt(), second.receipt());
    assert_eq!(fixture.inspect_fixture().as_ref().operation_count, 1);
}

#[test]
fn polling_sync_overlapping_windows_resubmit_safely() {
    let archetype = &ARCHETYPES[3];
    let fixture = B1ConformanceFixture::new();
    let enrollment = enroll(&fixture);
    let access = *enrollment.access();

    let window = |from: u32, to: u32| -> Vec<OperationId> {
        (from..to)
            .map(|item| derive_operation_id(&format!("simkl:history:{item}")))
            .collect()
    };

    for operation_id in window(0, 5) {
        fixture
            .accept_fixture(
                enrollment.credential_secret(),
                command(archetype, access, operation_id, "77"),
            )
            .expect("first window is accepted");
    }

    // The next poll overlaps the previous window, which is normal for a
    // timestamp-bounded diff pull.
    let mut replayed = 0;
    let mut committed = 0;
    for operation_id in window(3, 8) {
        let outcome = fixture
            .accept_fixture(
                enrollment.credential_secret(),
                command(archetype, access, operation_id, "77"),
            )
            .expect("overlapping window is accepted")
            .into_inner();
        match outcome {
            AcceptObservationOutcome::Committed(_) => committed += 1,
            AcceptObservationOutcome::Replayed(_) => replayed += 1,
        }
    }

    assert_eq!(replayed, 2, "the two overlapping items must replay");
    assert_eq!(committed, 3, "the three new items must commit");
    assert_eq!(
        fixture.inspect_fixture().as_ref().operation_count,
        8,
        "overlap must not inflate the operation set"
    );
}
