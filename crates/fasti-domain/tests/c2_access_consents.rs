//! Synthetic consent evidence checks, not grant or runtime authorization tests.

use chrono::{DateTime, TimeDelta, Utc};
use fasti_domain::*;

fn at(seconds: i64) -> DateTime<Utc> {
    DateTime::UNIX_EPOCH + TimeDelta::seconds(seconds)
}

fn digest(byte: u8) -> Sha256Digest {
    Sha256Digest::from_bytes(&[byte; 32])
}

fn subject(id: AuthSubjectId, lifecycle: AuthSubjectLifecycle, updated: i64) -> AuthSubject {
    AuthSubject::try_new(id, lifecycle, 0, 0, at(0), at(updated)).unwrap()
}

fn authority() -> (ApplicationClient, AuthSubject) {
    let owner = subject(AuthSubjectId::new_v7(), AuthSubjectLifecycle::Active, 0);
    let client = ApplicationClient::register(
        ClientId::new_v7(),
        WorkspaceId::new_v7(),
        owner.id(),
        AccessCredentialName::try_new("Synthetic integration").unwrap(),
        ApplicationClientPurpose::Integration,
        at(0),
    )
    .unwrap();
    (client, owner)
}

fn grant(
    client: &ApplicationClient,
    owner: &AuthSubject,
    time: i64,
) -> Result<AccessConsentRevision, AccessCredentialInvariantError> {
    AccessConsentRevision::grant(
        AccessConsentRevisionId::new_v7(),
        client,
        owner,
        ProfileId::new_v7(),
        ProfileGrantId::new_v7(),
        digest(1),
        at(time),
    )
}

fn restore(
    source: &AccessConsentRevision,
    sequence: u64,
    previous: Option<AccessConsentRevisionId>,
    decision: AccessConsentDecision,
) -> Result<AccessConsentRevision, AccessCredentialInvariantError> {
    AccessConsentRevision::try_from_persisted(
        source.id(),
        source.workspace_id(),
        source.client_id(),
        source.subject_id(),
        source.profile_id(),
        source.profile_grant_id(),
        sequence,
        previous,
        decision,
        source.created_at(),
    )
}

fn client_state(
    source: &ApplicationClient,
    id: ClientId,
    workspace: WorkspaceId,
    owner: Option<AuthSubjectId>,
    classification: ApplicationClientClassification,
    created: i64,
) -> ApplicationClient {
    ApplicationClient::try_from_persisted(
        id,
        workspace,
        owner,
        source.name().cloned(),
        classification,
        source.lifecycle(),
        source.current_credential_epoch(),
        at(created),
    )
    .unwrap()
}

fn assert_binding_preserved(previous: &AccessConsentRevision, next: &AccessConsentRevision) {
    assert_eq!(next.workspace_id(), previous.workspace_id());
    assert_eq!(next.client_id(), previous.client_id());
    assert_eq!(next.subject_id(), previous.subject_id());
    assert_eq!(next.profile_id(), previous.profile_id());
    assert_eq!(next.profile_grant_id(), previous.profile_grant_id());
    assert_eq!(next.previous_revision_id(), Some(previous.id()));
    assert_eq!(next.revision(), previous.revision() + 1);
}

#[test]
fn initial_grant_requires_current_person_owned_confidential_authority() {
    let (client, owner) = authority();
    let initial = grant(&client, &owner, 0).unwrap();
    assert_eq!(initial.revision(), 1);
    assert_eq!(initial.previous_revision_id(), None);
    assert_eq!(
        initial.decision(),
        &AccessConsentDecision::Granted(digest(1))
    );
    assert_eq!(initial.created_at(), at(0));
    assert_eq!(initial.client_id(), client.id());
    assert_eq!(initial.workspace_id(), client.workspace_id());
    assert_eq!(initial.subject_id(), owner.id());

    for lifecycle in [
        AuthSubjectLifecycle::Disabled,
        AuthSubjectLifecycle::Deleted,
        AuthSubjectLifecycle::RecoveryPending,
    ] {
        let inactive = subject(owner.id(), lifecycle, 0);
        assert_eq!(
            grant(&client, &inactive, 0),
            Err(AccessCredentialInvariantError::ConsentAuthorityMismatch)
        );
    }
    let other = subject(AuthSubjectId::new_v7(), AuthSubjectLifecycle::Active, 0);
    assert_eq!(
        grant(&client, &other, 0),
        Err(AccessCredentialInvariantError::ConsentAuthorityMismatch)
    );
    let future_owner = subject(owner.id(), AuthSubjectLifecycle::Active, 1);
    assert_eq!(
        grant(&client, &future_owner, 0),
        Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder)
    );
    assert!(grant(&client, &future_owner, 1).is_ok());
    assert!(grant(&client, &owner, -1).is_err());

    let node = ApplicationClientClassification::try_from_persisted(
        ClientAuthenticationType::FirstParty,
        ApplicationClientPurpose::Node,
    )
    .unwrap();
    for (creator, classification, created) in [
        (None, client.classification(), 0),
        (Some(owner.id()), node, 0),
        (Some(owner.id()), client.classification(), 1),
    ] {
        let invalid = client_state(
            &client,
            client.id(),
            client.workspace_id(),
            creator,
            classification,
            created,
        );
        assert!(grant(&invalid, &owner, 0).is_err());
    }
    let mut revoked = client;
    revoked.revoke();
    assert_eq!(
        grant(&revoked, &owner, 0),
        Err(AccessCredentialInvariantError::ClientRevoked)
    );
}

#[test]
fn persisted_sequences_require_positive_sqlite_range_and_valid_prior_links() {
    let (client, owner) = authority();
    let initial = grant(&client, &owner, 0).unwrap();
    let previous = AccessConsentRevisionId::new_v7();
    let max = i64::MAX as u64;
    for (sequence, prior, decision) in [
        (0, None, AccessConsentDecision::Granted(digest(1))),
        (
            max + 1,
            Some(previous),
            AccessConsentDecision::Granted(digest(1)),
        ),
        (
            u64::MAX,
            Some(previous),
            AccessConsentDecision::Granted(digest(1)),
        ),
        (1, Some(previous), AccessConsentDecision::Granted(digest(1))),
        (2, None, AccessConsentDecision::Granted(digest(1))),
        (
            2,
            Some(initial.id()),
            AccessConsentDecision::Granted(digest(1)),
        ),
        (1, None, AccessConsentDecision::Revoked),
    ] {
        assert_eq!(
            restore(&initial, sequence, prior, decision),
            Err(AccessCredentialInvariantError::InvalidConsentRevision)
        );
    }
    assert_eq!(
        restore(&initial, 1, None, initial.decision().clone()).unwrap(),
        initial
    );
    for sequence in [2, max] {
        for decision in [
            AccessConsentDecision::Granted(digest(1)),
            AccessConsentDecision::Revoked,
        ] {
            assert!(restore(&initial, sequence, Some(previous), decision).is_ok());
        }
    }
}

#[test]
fn granted_successors_are_immutable_bound_revisions_with_monotonic_time() {
    let (client, owner) = authority();
    let initial = grant(&client, &owner, 10).unwrap();
    let before = initial.clone();
    for time in [10, 11] {
        let id = AccessConsentRevisionId::new_v7();
        let next = initial
            .grant_successor(id, &client, &owner, digest(2), at(time))
            .unwrap();
        assert_binding_preserved(&initial, &next);
        assert_eq!(next.id(), id);
        assert_eq!(next.created_at(), at(time));
        assert_eq!(next.decision(), &AccessConsentDecision::Granted(digest(2)));
    }
    assert_eq!(
        initial.grant_successor(initial.id(), &client, &owner, digest(2), at(10)),
        Err(AccessCredentialInvariantError::InvalidConsentRevision)
    );
    assert_eq!(
        initial.grant_successor(
            AccessConsentRevisionId::new_v7(),
            &client,
            &owner,
            digest(2),
            at(9)
        ),
        Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder)
    );
    assert_eq!(initial, before);

    let max = i64::MAX as u64;
    for sequence in [max - 1, max] {
        let previous = restore(
            &initial,
            sequence,
            Some(AccessConsentRevisionId::new_v7()),
            initial.decision().clone(),
        )
        .unwrap();
        let snapshot = previous.clone();
        let granted = previous.grant_successor(
            AccessConsentRevisionId::new_v7(),
            &client,
            &owner,
            digest(2),
            at(11),
        );
        let revoked = previous.revoke_successor(AccessConsentRevisionId::new_v7(), at(11));
        if sequence == max - 1 {
            assert_eq!(granted.unwrap().revision(), max);
            assert_eq!(revoked.unwrap().unwrap().revision(), max);
        } else {
            assert_eq!(
                granted,
                Err(AccessCredentialInvariantError::InvalidConsentRevision)
            );
            assert_eq!(
                revoked,
                Err(AccessCredentialInvariantError::InvalidConsentRevision)
            );
        }
        assert_eq!(previous, snapshot);
    }
}

#[test]
fn granted_successor_rejects_client_workspace_and_subject_mismatches() {
    let (client, owner) = authority();
    let initial = grant(&client, &owner, 0).unwrap();
    let other_owner = subject(AuthSubjectId::new_v7(), AuthSubjectLifecycle::Active, 0);
    for (id, workspace, approver) in [
        (ClientId::new_v7(), client.workspace_id(), &owner),
        (client.id(), WorkspaceId::new_v7(), &owner),
        (client.id(), client.workspace_id(), &other_owner),
    ] {
        // Each altered client is internally valid and matches its approver;
        // the original consent binding must still reject the successor.
        let changed = client_state(
            &client,
            id,
            workspace,
            Some(approver.id()),
            client.classification(),
            0,
        );
        assert_eq!(
            initial.grant_successor(
                AccessConsentRevisionId::new_v7(),
                &changed,
                approver,
                digest(2),
                at(0)
            ),
            Err(AccessCredentialInvariantError::ConsentAuthorityMismatch)
        );
    }
    let inactive_owner = subject(owner.id(), AuthSubjectLifecycle::Disabled, 0);
    assert!(initial
        .grant_successor(
            AccessConsentRevisionId::new_v7(),
            &client,
            &inactive_owner,
            digest(2),
            at(0)
        )
        .is_err());
    let mut revoked_client = client;
    revoked_client.revoke();
    assert!(initial
        .grant_successor(
            AccessConsentRevisionId::new_v7(),
            &revoked_client,
            &owner,
            digest(2),
            at(0)
        )
        .is_err());
}

#[test]
fn withdrawal_appends_once_and_the_withdrawn_chain_cannot_regrant() {
    let (client, owner) = authority();
    let initial = grant(&client, &owner, 10).unwrap();
    let before = initial.clone();
    assert_eq!(
        initial.revoke_successor(initial.id(), at(10)),
        Err(AccessCredentialInvariantError::InvalidConsentRevision)
    );
    assert_eq!(
        initial.revoke_successor(AccessConsentRevisionId::new_v7(), at(9)),
        Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder)
    );
    let withdrawn = initial
        .revoke_successor(AccessConsentRevisionId::new_v7(), at(10))
        .unwrap()
        .unwrap();
    assert_binding_preserved(&initial, &withdrawn);
    assert_eq!(withdrawn.decision(), &AccessConsentDecision::Revoked);
    assert_eq!(withdrawn.created_at(), at(10));
    assert_eq!(initial, before);
    let terminal = withdrawn.clone();
    for time in [10, 11] {
        assert_eq!(
            withdrawn.revoke_successor(AccessConsentRevisionId::new_v7(), at(time)),
            Ok(None)
        );
        assert_eq!(
            withdrawn.grant_successor(
                AccessConsentRevisionId::new_v7(),
                &client,
                &owner,
                digest(2),
                at(time)
            ),
            Err(AccessCredentialInvariantError::ConsentRevoked)
        );
    }
    assert_eq!(
        withdrawn.revoke_successor(AccessConsentRevisionId::new_v7(), at(9)),
        Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder)
    );
    assert_eq!(withdrawn, terminal);
    let exhausted = restore(
        &withdrawn,
        i64::MAX as u64,
        Some(initial.id()),
        AccessConsentDecision::Revoked,
    )
    .unwrap();
    assert_eq!(
        exhausted.revoke_successor(AccessConsentRevisionId::new_v7(), at(11)),
        Ok(None)
    );
}
