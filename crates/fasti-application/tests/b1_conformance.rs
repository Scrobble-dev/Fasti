#![cfg(feature = "conformance-fixture")]

use fasti_application::{
    conformance::{B1ConformanceFixture, FixtureDurability, FixturePhase},
    AcceptObservationCommand, AcceptObservationOutcome, ProblemCode, ReplayReceiptQuery,
};
use fasti_domain::{
    ClaimedTrust, EvidenceId, EvidenceReference, ObservedAt, OperationId, RequestCorrelationId,
    Sha256Digest,
};
use static_assertions::assert_not_impl_any;
use std::sync::{Arc, Barrier};

assert_not_impl_any!(fasti_application::conformance::FixtureCredentialSecret: Clone, std::fmt::Debug, serde::Serialize);

fn initialize_and_enroll(
    fixture: &B1ConformanceFixture,
) -> fasti_application::conformance::FixtureEnrollment {
    let initialization = fixture
        .initialize_node(RequestCorrelationId::new_v7())
        .expect("fresh fixture initializes")
        .into_inner();
    fixture
        .enroll_first_client(RequestCorrelationId::new_v7(), &initialization)
        .expect("first client enrolls")
        .into_inner()
}

fn command(
    access: fasti_application::RequestAccessContext,
    operation_id: OperationId,
    digest_byte: &str,
) -> AcceptObservationCommand {
    AcceptObservationCommand::new(
        RequestCorrelationId::new_v7(),
        access,
        operation_id,
        None,
        ObservedAt::parse("2026-08-22T10:11:12Z", ClaimedTrust::DeviceObserved)
            .expect("valid observed time"),
        EvidenceReference::new(
            EvidenceId::new_v7(),
            Sha256Digest::parse(format!("sha256:{}", digest_byte.repeat(32)))
                .expect("canonical digest"),
            32,
        ),
    )
}

#[test]
fn fixture_starts_empty_and_every_direct_result_disclaims_durability() {
    let fixture = B1ConformanceFixture::new();
    let initial = fixture.inspect_fixture();
    assert_eq!(initial.durability(), FixtureDurability::None);
    assert!(initial.is_fixture_only());
    assert_eq!(initial.as_ref().phase, FixturePhase::Empty);

    let initialization = fixture
        .initialize_node(RequestCorrelationId::new_v7())
        .expect("initialize");
    assert_eq!(initialization.durability(), FixtureDurability::None);
    let enrollment = fixture
        .enroll_first_client(RequestCorrelationId::new_v7(), initialization.as_ref())
        .expect("enroll");
    assert_eq!(enrollment.durability(), FixtureDurability::None);

    let acceptance = fixture
        .accept_fixture(
            enrollment.as_ref().credential_secret(),
            command(*enrollment.as_ref().access(), OperationId::new_v7(), "11"),
        )
        .expect("accept");
    assert_eq!(acceptance.durability(), FixtureDurability::None);
    let replay = fixture
        .replay_fixture(
            enrollment.as_ref().credential_secret(),
            ReplayReceiptQuery::new(
                RequestCorrelationId::new_v7(),
                *enrollment.as_ref().access(),
                acceptance.as_ref().receipt().receipt_id(),
            ),
        )
        .expect("replay");
    assert_eq!(replay.durability(), FixtureDurability::None);
}

#[test]
fn initialize_race_has_exactly_one_winner() {
    let fixture = Arc::new(B1ConformanceFixture::new());
    let barrier = Arc::new(Barrier::new(8));
    let handles = (0..8)
        .map(|_| {
            let fixture = Arc::clone(&fixture);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                fixture.initialize_node(RequestCorrelationId::new_v7())
            })
        })
        .collect::<Vec<_>>();

    let results = handles
        .into_iter()
        .map(|handle| handle.join().expect("thread does not panic"))
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert!(results
        .iter()
        .filter_map(|result| result.as_ref().err())
        .all(|problem| problem.code() == ProblemCode::Forbidden));
}

#[test]
fn first_client_enrollment_is_single_use_and_issues_fresh_redacted_secrets() {
    let fixture_a = B1ConformanceFixture::new();
    let init_a = fixture_a
        .initialize_node(RequestCorrelationId::new_v7())
        .expect("initialize A")
        .into_inner();
    let enrollment_a = fixture_a
        .enroll_first_client(RequestCorrelationId::new_v7(), &init_a)
        .expect("enroll A")
        .into_inner();
    let duplicate = fixture_a.enroll_first_client(RequestCorrelationId::new_v7(), &init_a);
    assert_eq!(
        duplicate.expect_err("enrollment is single-use").code(),
        ProblemCode::Forbidden
    );
    assert!(format!("{enrollment_a:?}").contains("[REDACTED]"));

    let fixture_b = B1ConformanceFixture::new();
    let enrollment_b = initialize_and_enroll(&fixture_b);
    assert_ne!(
        enrollment_a.credential_secret().expose_for_fixture(),
        enrollment_b.credential_secret().expose_for_fixture(),
        "each enrollment must issue fresh credential material"
    );
}

#[test]
fn authorization_is_rechecked_and_mismatched_access_cannot_accept() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = initialize_and_enroll(&fixture);
    let other_fixture = B1ConformanceFixture::new();
    let other_enrollment = initialize_and_enroll(&other_fixture);

    let error = fixture
        .accept_fixture(
            enrollment.credential_secret(),
            command(*other_enrollment.access(), OperationId::new_v7(), "22"),
        )
        .expect_err("foreign access must be denied");
    assert_eq!(error.code(), ProblemCode::Forbidden);
    assert_eq!(
        fixture.inspect_fixture().as_ref().operation_count,
        0,
        "denial occurs without mutation"
    );

    let wrong_secret = fixture
        .accept_fixture(
            other_enrollment.credential_secret(),
            command(*enrollment.access(), OperationId::new_v7(), "23"),
        )
        .expect_err("foreign credential material must be denied");
    assert_eq!(wrong_secret.code(), ProblemCode::Forbidden);
    assert_eq!(fixture.inspect_fixture().as_ref().operation_count, 0);
}

#[test]
fn idempotency_and_receipt_replay_are_atomic_and_exact() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = initialize_and_enroll(&fixture);
    let access = *enrollment.access();
    let operation_id = OperationId::new_v7();

    let committed = fixture
        .accept_fixture(
            enrollment.credential_secret(),
            command(access, operation_id, "33"),
        )
        .expect("first acceptance")
        .into_inner();
    assert!(matches!(committed, AcceptObservationOutcome::Committed(_)));
    let original = committed.receipt().clone();

    let exact_replay = fixture
        .accept_fixture(
            enrollment.credential_secret(),
            command(access, operation_id, "33"),
        )
        .expect("same operation and digest replays")
        .into_inner();
    assert!(exact_replay.is_replay());
    assert_eq!(exact_replay.receipt(), &original);

    let changed = fixture
        .accept_fixture(
            enrollment.credential_secret(),
            command(access, operation_id, "44"),
        )
        .expect_err("changed digest conflicts");
    assert_eq!(changed.code(), ProblemCode::IdempotencyConflict);

    let replayed_original = fixture
        .replay_fixture(
            enrollment.credential_secret(),
            ReplayReceiptQuery::new(
                RequestCorrelationId::new_v7(),
                access,
                original.receipt_id(),
            ),
        )
        .expect("original receipt remains available")
        .into_inner();
    assert_eq!(replayed_original, original);
    assert_eq!(fixture.inspect_fixture().as_ref().operation_count, 1);
    assert_eq!(fixture.inspect_fixture().as_ref().receipt_count, 1);
}

#[test]
fn contended_same_operation_commits_once_and_replays_one_exact_receipt() {
    let fixture = Arc::new(B1ConformanceFixture::new());
    let enrollment = Arc::new(initialize_and_enroll(&fixture));
    let access = *enrollment.access();
    let operation_id = OperationId::new_v7();
    let barrier = Arc::new(Barrier::new(8));
    let handles = (0..8)
        .map(|_| {
            let fixture = Arc::clone(&fixture);
            let barrier = Arc::clone(&barrier);
            let enrollment = Arc::clone(&enrollment);
            std::thread::spawn(move || {
                barrier.wait();
                fixture
                    .accept_fixture(
                        enrollment.credential_secret(),
                        command(access, operation_id, "66"),
                    )
                    .expect("contended identical request succeeds")
                    .into_inner()
            })
        })
        .collect::<Vec<_>>();
    let outcomes = handles
        .into_iter()
        .map(|handle| handle.join().expect("thread does not panic"))
        .collect::<Vec<_>>();

    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| !outcome.is_replay())
            .count(),
        1
    );
    let receipt = outcomes[0].receipt();
    assert!(outcomes.iter().all(|outcome| outcome.receipt() == receipt));
    assert_eq!(fixture.inspect_fixture().as_ref().operation_count, 1);
    assert_eq!(fixture.inspect_fixture().as_ref().receipt_count, 1);
}

#[test]
fn same_digest_with_new_operation_creates_distinct_unresolved_receipt() {
    let fixture = B1ConformanceFixture::new();
    let enrollment = initialize_and_enroll(&fixture);
    let access = *enrollment.access();

    let first = fixture
        .accept_fixture(
            enrollment.credential_secret(),
            command(access, OperationId::new_v7(), "55"),
        )
        .expect("first")
        .into_inner();
    let second = fixture
        .accept_fixture(
            enrollment.credential_secret(),
            command(access, OperationId::new_v7(), "55"),
        )
        .expect("second")
        .into_inner();

    assert_ne!(
        first.receipt().operation_id(),
        second.receipt().operation_id()
    );
    assert_ne!(
        first.receipt().observation_id(),
        second.receipt().observation_id()
    );
    assert_ne!(first.receipt().receipt_id(), second.receipt().receipt_id());
    let value = serde_json::to_value(second.receipt()).expect("serialize receipt");
    assert!(value.get("record_id").is_none());
    assert!(value.get("occurrence_id").is_none());
    assert_eq!(fixture.inspect_fixture().as_ref().operation_count, 2);
    assert_eq!(fixture.inspect_fixture().as_ref().receipt_count, 2);
}
