//! Synthetic domain history checks; these identities establish no authority.

use fasti_domain::{
    AuthSubjectId, FirstOidcLinkEligibility, FirstOidcLinkError, OperationId, TrailBaseInstanceId,
};

#[test]
fn only_the_original_administrator_can_consume_installation_eligibility() {
    let installation = TrailBaseInstanceId::new_v7();
    let original_administrator = AuthSubjectId::new_v7();
    let mut eligibility =
        FirstOidcLinkEligibility::from_original_bootstrap(installation, original_administrator);
    assert_eq!(eligibility.trailbase_instance_id(), installation);
    assert_eq!(
        eligibility.original_administrator_subject_id(),
        original_administrator
    );
    assert_eq!(eligibility.consumed_by(), None);

    let before = eligibility.clone();
    assert_eq!(
        eligibility.consume(AuthSubjectId::new_v7(), OperationId::new_v7()),
        Err(FirstOidcLinkError::SubjectMismatch)
    );
    assert_eq!(eligibility, before);

    let operation = OperationId::new_v7();
    assert_eq!(
        eligibility.consume(original_administrator, operation),
        Ok(())
    );
    assert_eq!(eligibility.consumed_by(), Some(operation));
    assert_eq!(eligibility.trailbase_instance_id(), installation);
    assert_eq!(
        eligibility.original_administrator_subject_id(),
        original_administrator
    );
}

#[test]
fn repeated_consumption_and_reconstruction_preserve_the_original_consumption() {
    let original_administrator = AuthSubjectId::new_v7();
    let mut eligibility = FirstOidcLinkEligibility::from_original_bootstrap(
        TrailBaseInstanceId::new_v7(),
        original_administrator,
    );
    let first_operation = OperationId::new_v7();
    eligibility
        .consume(original_administrator, first_operation)
        .unwrap();
    let consumed = eligibility.clone();
    let reconstructed = FirstOidcLinkEligibility::from_persisted(
        eligibility.trailbase_instance_id(),
        eligibility.original_administrator_subject_id(),
        eligibility.consumed_by(),
    );
    assert_eq!(reconstructed, consumed);

    for mut record in [eligibility, reconstructed] {
        for operation in [first_operation, OperationId::new_v7()] {
            assert_eq!(
                record.consume(original_administrator, operation),
                Err(FirstOidcLinkError::AlreadyConsumed)
            );
            assert_eq!(record, consumed);
        }
        assert_eq!(
            record.consume(AuthSubjectId::new_v7(), OperationId::new_v7()),
            Err(FirstOidcLinkError::SubjectMismatch)
        );
        assert_eq!(record, consumed);
    }
}

#[test]
fn reconstructing_unused_history_preserves_the_subject_binding() {
    let original_administrator = AuthSubjectId::new_v7();
    let original = FirstOidcLinkEligibility::from_original_bootstrap(
        TrailBaseInstanceId::new_v7(),
        original_administrator,
    );
    let mut reconstructed = FirstOidcLinkEligibility::from_persisted(
        original.trailbase_instance_id(),
        original.original_administrator_subject_id(),
        original.consumed_by(),
    );
    assert_eq!(reconstructed, original);
    assert_eq!(
        reconstructed.consume(AuthSubjectId::new_v7(), OperationId::new_v7()),
        Err(FirstOidcLinkError::SubjectMismatch)
    );
    assert_eq!(reconstructed, original);
    let operation = OperationId::new_v7();
    reconstructed
        .consume(original_administrator, operation)
        .unwrap();
    assert_eq!(reconstructed.consumed_by(), Some(operation));
}
