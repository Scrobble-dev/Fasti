//! Pure synthetic domain checks; these fixtures establish no runtime authority.

use chrono::{DateTime, TimeDelta, Utc};
use fasti_domain::*;

fn at(seconds: i64) -> DateTime<Utc> {
    DateTime::UNIX_EPOCH + TimeDelta::seconds(seconds)
}

fn authority() -> (AuthSubject, TrailBaseInstallation) {
    let subject = AuthSubject::try_new(
        AuthSubjectId::new_v7(),
        AuthSubjectLifecycle::Active,
        0,
        0,
        at(0),
        at(0),
    )
    .unwrap();
    let mut installation = TrailBaseInstallation::new(
        TrailBaseInstanceId::new_v7(),
        Sha256Digest::from_bytes(&[1; 32]),
        Sha256Digest::from_bytes(&[2; 32]),
        at(0),
    );
    installation
        .verify(
            &Sha256Digest::from_bytes(&[1; 32]),
            &Sha256Digest::from_bytes(&[2; 32]),
            at(0),
        )
        .unwrap();
    (subject, installation)
}

fn issue(
    subject: &AuthSubject,
    installation: &TrailBaseInstallation,
    created: DateTime<Utc>,
) -> PersonalAccessToken {
    PersonalAccessToken::issue(
        PersonalAccessTokenId::new_v7(),
        WorkspaceId::new_v7(),
        ProfileGrantId::new_v7(),
        AccessCredentialName::try_new("Synthetic PAT").unwrap(),
        Sha256Digest::from_bytes(&[3; 32]),
        subject,
        installation,
        created,
        created + TimeDelta::days(1),
    )
    .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn restore(
    token: &PersonalAccessToken,
    epochs: [u64; 3],
    created: DateTime<Utc>,
    expires: DateTime<Utc>,
    last_used: Option<DateTime<Utc>>,
    revoked: Option<DateTime<Utc>>,
    replaced_by: Option<PersonalAccessTokenId>,
) -> Result<PersonalAccessToken, AccessCredentialInvariantError> {
    PersonalAccessToken::try_from_persisted(
        token.id(),
        token.workspace_id(),
        token.subject_id(),
        token.profile_grant_id(),
        token.name().clone(),
        token.digest().clone(),
        epochs[0],
        epochs[1],
        token.trailbase_instance_id(),
        epochs[2],
        created,
        expires,
        last_used,
        revoked,
        replaced_by,
    )
}

fn subject_state(
    subject: &AuthSubject,
    lifecycle: AuthSubjectLifecycle,
    epochs: [u64; 2],
    updated: DateTime<Utc>,
) -> AuthSubject {
    AuthSubject::try_new(
        subject.id(),
        lifecycle,
        epochs[0],
        epochs[1],
        subject.created_at(),
        updated,
    )
    .unwrap()
}

fn installation_state(
    installation: &TrailBaseInstallation,
    state: TrailBaseActivationState,
    generation: u64,
    updated: DateTime<Utc>,
) -> TrailBaseInstallation {
    TrailBaseInstallation::try_from_persisted(
        installation.id(),
        installation.physical_root_identity().clone(),
        installation.release_lock_identity().cloned(),
        state,
        generation,
        installation.created_at(),
        updated,
    )
    .unwrap()
}

#[test]
fn exact_expiry_and_monotonic_use_and_revocation_preserve_terminal_state() {
    let (subject, installation) = authority();
    let mut token = issue(&subject, &installation, at(0));
    let tick = TimeDelta::nanoseconds(1);
    let expiry = token.expires_at();
    assert!(!token.is_current_for(&subject, &installation, at(0) - tick));
    assert!(token.is_current_for(&subject, &installation, at(0)));
    assert!(token.is_current_for(&subject, &installation, expiry - tick));
    assert!(!token.is_current_for(&subject, &installation, expiry));
    assert!(!token.is_current_for(&subject, &installation, expiry + tick));
    let original = token.clone();
    for invalid in [at(0) - tick, expiry, expiry + tick] {
        assert_eq!(
            token.record_use(invalid),
            Err(AccessCredentialInvariantError::PersonalAccessTokenUnavailable)
        );
        assert_eq!(token, original);
    }
    assert_eq!(token.record_use(at(1)), Ok(true));
    assert_eq!(token.record_use(at(1)), Ok(false));
    assert_eq!(token.record_use(at(2)), Ok(true));
    assert_eq!(token.last_used_at(), Some(at(2)));
    let used = token.clone();
    assert!(token.record_use(at(1)).is_err());
    assert_eq!(
        token.revoke(at(1)),
        Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder)
    );
    assert_eq!(token, used);
    assert!(!token.is_current_for(&subject, &installation, at(1)));
    // An expired PAT remains inspectable and revocable.
    assert_eq!(token.revoke(expiry), Ok(true));
    let terminal = token.clone();
    assert_eq!(token.revoke(expiry), Ok(false));
    assert_eq!(token.revoke(expiry + tick), Ok(false));
    assert!(token.revoke(expiry - tick).is_err());
    assert!(token.record_use(expiry + tick).is_err());
    assert!(!token.is_current_for(&subject, &installation, at(2)));
    assert_eq!(token, terminal);
    assert_eq!(token.id(), original.id());
    assert_eq!(token.digest(), original.digest());
    assert_eq!(token.expires_at(), original.expires_at());
    assert_eq!(token.last_used_at(), Some(at(2)));
    assert_eq!(token.revoked_at(), Some(expiry));
    assert_eq!(token.replaced_by(), None);
}

#[test]
fn persistence_rejects_epoch_overflow_and_inconsistent_timestamps_or_replacement() {
    let (subject, installation) = authority();
    let token = issue(&subject, &installation, at(0));
    let max = i64::MAX as u64;
    for epochs in [[0, 0, 1], [max, max, max]] {
        assert!(restore(&token, epochs, at(0), at(100), None, None, None).is_ok());
    }
    for epochs in [
        [0, 0, 0],
        [max + 1, 0, 1],
        [0, max + 1, 1],
        [0, 0, max + 1],
        [u64::MAX, u64::MAX, u64::MAX],
    ] {
        assert_eq!(
            restore(&token, epochs, at(0), at(100), None, None, None),
            Err(AccessCredentialInvariantError::InvalidCredentialEpoch)
        );
    }
    for (expiry, last_used, revoked) in [
        (at(0), None, None),
        (at(-1), None, None),
        (at(100), Some(at(-1)), None),
        (at(100), Some(at(100)), None),
        (at(100), Some(at(101)), None),
        (at(100), None, Some(at(-1))),
        (at(100), Some(at(2)), Some(at(1))),
    ] {
        assert_eq!(
            restore(&token, [0, 0, 1], at(0), expiry, last_used, revoked, None),
            Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder)
        );
    }
    for (revoked, replacement) in [
        (None, PersonalAccessTokenId::new_v7()),
        (Some(at(1)), token.id()),
    ] {
        assert_eq!(
            restore(
                &token,
                [0, 0, 1],
                at(0),
                at(100),
                None,
                revoked,
                Some(replacement)
            ),
            Err(AccessCredentialInvariantError::InvalidPersonalAccessTokenReplacement)
        );
    }
    let restored = restore(
        &token,
        [0, 0, 1],
        at(0),
        at(100),
        Some(at(0)),
        Some(at(100)),
        Some(PersonalAccessTokenId::new_v7()),
    )
    .unwrap();
    assert!(!restored.is_current_for(&subject, &installation, at(1)));
}

#[test]
fn issuance_checks_current_authority_time_and_sqlite_epoch_ceilings() {
    let (subject, installation) = authority();
    let attempt =
        |subject: &AuthSubject, installation: &TrailBaseInstallation, created, expires| {
            PersonalAccessToken::issue(
                PersonalAccessTokenId::new_v7(),
                WorkspaceId::new_v7(),
                ProfileGrantId::new_v7(),
                AccessCredentialName::try_new("Synthetic PAT").unwrap(),
                Sha256Digest::from_bytes(&[3; 32]),
                subject,
                installation,
                created,
                expires,
            )
        };
    for lifecycle in [
        AuthSubjectLifecycle::Disabled,
        AuthSubjectLifecycle::Deleted,
        AuthSubjectLifecycle::RecoveryPending,
    ] {
        let inactive = subject_state(&subject, lifecycle, [0, 0], at(0));
        assert_eq!(
            attempt(&inactive, &installation, at(0), at(100)),
            Err(AccessCredentialInvariantError::PersonalAccessTokenUnavailable)
        );
    }
    for (state, generation) in [
        (TrailBaseActivationState::Inactive, 0),
        (
            TrailBaseActivationState::Blocked(TrailBaseActivationBlocker::ReleaseMismatch),
            1,
        ),
        (
            TrailBaseActivationState::Blocked(
                TrailBaseActivationBlocker::PhysicalRootIdentityMismatch,
            ),
            1,
        ),
        (
            TrailBaseActivationState::Blocked(TrailBaseActivationBlocker::DeclaredRestore),
            1,
        ),
    ] {
        let inactive = installation_state(&installation, state, generation, at(0));
        assert_eq!(
            attempt(&subject, &inactive, at(0), at(100)),
            Err(AccessCredentialInvariantError::PersonalAccessTokenUnavailable)
        );
    }
    let future_subject = subject_state(&subject, AuthSubjectLifecycle::Active, [0, 0], at(1));
    let future_installation =
        installation_state(&installation, TrailBaseActivationState::Active, 1, at(1));
    for (subject, installation, created, expires) in [
        (&future_subject, &installation, at(0), at(100)),
        (&subject, &future_installation, at(0), at(100)),
        (&subject, &installation, at(0), at(0)),
        (&subject, &installation, at(1), at(0)),
    ] {
        assert_eq!(
            attempt(subject, installation, created, expires),
            Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder)
        );
    }
    let max = i64::MAX as u64;
    let max_subject = subject_state(&subject, AuthSubjectLifecycle::Active, [max, max], at(0));
    let max_installation =
        installation_state(&installation, TrailBaseActivationState::Active, max, at(0));
    assert!(attempt(&max_subject, &max_installation, at(0), at(100)).is_ok());
    for epochs in [[max + 1, 0], [0, max + 1]] {
        let oversized = subject_state(&subject, AuthSubjectLifecycle::Active, epochs, at(0));
        assert_eq!(
            attempt(&oversized, &installation, at(0), at(100)),
            Err(AccessCredentialInvariantError::InvalidCredentialEpoch)
        );
    }
    let oversized = installation_state(
        &installation,
        TrailBaseActivationState::Active,
        max + 1,
        at(0),
    );
    assert_eq!(
        attempt(&subject, &oversized, at(0), at(100)),
        Err(AccessCredentialInvariantError::InvalidCredentialEpoch)
    );
}

#[test]
fn current_state_rejects_mismatched_or_inactive_authority_and_reactivation() {
    let (subject, mut installation) = authority();
    let token = issue(&subject, &installation, at(0));
    let (other_subject, other_installation) = authority();
    assert!(!token.is_current_for(&other_subject, &installation, at(0)));
    assert!(!token.is_current_for(&subject, &other_installation, at(0)));
    for (lifecycle, epochs, updated) in [
        (AuthSubjectLifecycle::Disabled, [0, 0], at(0)),
        (AuthSubjectLifecycle::Deleted, [0, 0], at(0)),
        (AuthSubjectLifecycle::RecoveryPending, [0, 0], at(0)),
        (AuthSubjectLifecycle::Active, [1, 0], at(0)),
        (AuthSubjectLifecycle::Active, [0, 1], at(0)),
        (AuthSubjectLifecycle::Active, [0, 0], at(1)),
    ] {
        let changed = subject_state(&subject, lifecycle, epochs, updated);
        assert!(!token.is_current_for(&changed, &installation, at(0)));
    }
    for (state, generation, updated) in [
        (TrailBaseActivationState::Inactive, 0, at(0)),
        (TrailBaseActivationState::Active, 2, at(0)),
        (TrailBaseActivationState::Active, 1, at(1)),
        (
            TrailBaseActivationState::Blocked(TrailBaseActivationBlocker::DeclaredRestore),
            1,
            at(0),
        ),
    ] {
        let changed = installation_state(&installation, state, generation, updated);
        assert!(!token.is_current_for(&subject, &changed, at(0)));
    }
    installation
        .verify(
            &Sha256Digest::from_bytes(&[1; 32]),
            &Sha256Digest::from_bytes(&[9; 32]),
            at(1),
        )
        .unwrap();
    assert!(!token.is_current_for(&subject, &installation, at(1)));
    installation
        .verify(
            &Sha256Digest::from_bytes(&[1; 32]),
            &Sha256Digest::from_bytes(&[2; 32]),
            at(2),
        )
        .unwrap();
    assert_eq!(
        installation.activation_state(),
        TrailBaseActivationState::Active
    );
    assert!(installation.activation_generation() > token.activation_generation());
    assert!(!token.is_current_for(&subject, &installation, at(2)));
}

#[test]
fn replacement_rejects_wrong_binding_reused_identity_or_digest_and_nonfresh_state() {
    let (subject, installation) = authority();
    let original = issue(&subject, &installation, at(0));
    let candidate = |id, workspace, grant, digest, owner: &AuthSubject, created| {
        PersonalAccessToken::issue(
            id,
            workspace,
            grant,
            original.name().clone(),
            digest,
            owner,
            &installation,
            created,
            created + TimeDelta::days(1),
        )
        .unwrap()
    };
    let successor = candidate(
        PersonalAccessTokenId::new_v7(),
        original.workspace_id(),
        original.profile_grant_id(),
        Sha256Digest::from_bytes(&[4; 32]),
        &subject,
        at(10),
    );
    let (other_subject, _) = authority();
    for (id, workspace, grant, digest, owner, created) in [
        (
            original.id(),
            original.workspace_id(),
            original.profile_grant_id(),
            successor.digest().clone(),
            &subject,
            at(10),
        ),
        (
            successor.id(),
            WorkspaceId::new_v7(),
            original.profile_grant_id(),
            successor.digest().clone(),
            &subject,
            at(10),
        ),
        (
            successor.id(),
            original.workspace_id(),
            ProfileGrantId::new_v7(),
            successor.digest().clone(),
            &subject,
            at(10),
        ),
        (
            successor.id(),
            original.workspace_id(),
            original.profile_grant_id(),
            original.digest().clone(),
            &subject,
            at(10),
        ),
        (
            successor.id(),
            original.workspace_id(),
            original.profile_grant_id(),
            successor.digest().clone(),
            &other_subject,
            at(10),
        ),
        (
            successor.id(),
            original.workspace_id(),
            original.profile_grant_id(),
            successor.digest().clone(),
            &subject,
            at(9),
        ),
        (
            successor.id(),
            original.workspace_id(),
            original.profile_grant_id(),
            successor.digest().clone(),
            &subject,
            at(11),
        ),
    ] {
        let invalid = candidate(id, workspace, grant, digest, owner, created);
        let mut predecessor = original.clone();
        assert_eq!(
            predecessor.replace_with(&invalid, at(10)),
            Err(AccessCredentialInvariantError::InvalidPersonalAccessTokenReplacement)
        );
        assert_eq!(predecessor, original);
    }
    for (last_used, revoked, replaced) in [
        (Some(at(10)), None, None),
        (None, Some(at(10)), None),
        (None, Some(at(10)), Some(PersonalAccessTokenId::new_v7())),
    ] {
        let invalid = restore(
            &successor,
            [0, 0, 1],
            at(10),
            at(100),
            last_used,
            revoked,
            replaced,
        )
        .unwrap();
        let mut predecessor = original.clone();
        assert!(predecessor.replace_with(&invalid, at(10)).is_err());
        assert_eq!(predecessor, original);
    }
    let mut used = original.clone();
    used.record_use(at(11)).unwrap();
    let before = used.clone();
    assert_eq!(
        used.replace_with(&successor, at(10)),
        Err(AccessCredentialInvariantError::InvalidCredentialTimestampOrder)
    );
    assert_eq!(used, before);
    let mut revoked = original;
    revoked.revoke(at(1)).unwrap();
    let before = revoked.clone();
    assert!(revoked.replace_with(&successor, at(10)).is_err());
    assert_eq!(revoked, before);
}

#[test]
fn expired_predecessor_can_be_replaced_with_fresh_current_authority_only_once() {
    let (mut subject, mut installation) = authority();
    let mut predecessor = issue(&subject, &installation, at(0));
    let original = predecessor.clone();
    let replacement_time = predecessor.expires_at();
    subject.advance_authentication_epoch(at(1)).unwrap();
    subject.advance_authorization_epoch(at(1)).unwrap();
    installation
        .verify(
            &Sha256Digest::from_bytes(&[1; 32]),
            &Sha256Digest::from_bytes(&[9; 32]),
            at(1),
        )
        .unwrap();
    installation
        .verify(
            &Sha256Digest::from_bytes(&[1; 32]),
            &Sha256Digest::from_bytes(&[2; 32]),
            at(2),
        )
        .unwrap();
    let replacement = PersonalAccessToken::issue(
        PersonalAccessTokenId::new_v7(),
        predecessor.workspace_id(),
        predecessor.profile_grant_id(),
        predecessor.name().clone(),
        Sha256Digest::from_bytes(&[4; 32]),
        &subject,
        &installation,
        replacement_time,
        replacement_time + TimeDelta::days(1),
    )
    .unwrap();
    assert!(!predecessor.is_current_for(&subject, &installation, replacement_time));
    assert!(replacement.is_current_for(&subject, &installation, replacement_time));
    assert_eq!(replacement.auth_epoch(), subject.auth_epoch());
    assert_eq!(
        replacement.authorization_epoch(),
        subject.authorization_epoch()
    );
    assert_eq!(replacement.trailbase_instance_id(), installation.id());
    assert_eq!(
        replacement.activation_generation(),
        installation.activation_generation()
    );
    assert_eq!(
        predecessor.replace_with(&replacement, replacement_time),
        Ok(())
    );
    assert_eq!(predecessor.revoked_at(), Some(replacement_time));
    assert_eq!(predecessor.replaced_by(), Some(replacement.id()));
    assert_eq!(predecessor.digest(), original.digest());
    assert_eq!(predecessor.auth_epoch(), original.auth_epoch());
    assert_eq!(
        predecessor.activation_generation(),
        original.activation_generation()
    );
    let terminal = predecessor.clone();
    assert!(predecessor
        .replace_with(&replacement, replacement_time)
        .is_err());
    assert!(predecessor.record_use(replacement_time).is_err());
    assert_eq!(
        predecessor.revoke(replacement_time + TimeDelta::seconds(1)),
        Ok(false)
    );
    assert_eq!(predecessor, terminal);
    assert!(!predecessor.is_current_for(&subject, &installation, replacement_time));
    assert_eq!(replacement.revoked_at(), None);
    assert_eq!(replacement.replaced_by(), None);
    assert_eq!(replacement.last_used_at(), None);
}
