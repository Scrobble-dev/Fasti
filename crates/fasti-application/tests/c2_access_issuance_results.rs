//! Synthetic result consistency checks, not commit or runtime authority proof.

use chrono::{DateTime, TimeDelta, Utc};
use fasti_application::*;
use fasti_domain::*;
use static_assertions::assert_not_impl_any;
use std::fmt;

assert_not_impl_any!(IssuedAccessClientCredential:
    fmt::Debug, Clone, Default, serde::Serialize, serde::de::DeserializeOwned,
    From<RequestAccessContext>, From<ApplicationAccessContext>, From<AuthenticatedBrowserSession>);
assert_not_impl_any!(IssuedPersonalAccessToken:
    fmt::Debug, Clone, Default, serde::Serialize, serde::de::DeserializeOwned,
    From<RequestAccessContext>, From<ApplicationAccessContext>, From<AuthenticatedBrowserSession>);

fn at(seconds: i64) -> DateTime<Utc> {
    DateTime::UNIX_EPOCH + TimeDelta::seconds(seconds)
}

fn owner() -> AuthSubject {
    AuthSubject::try_new(
        AuthSubjectId::new_v7(),
        AuthSubjectLifecycle::Active,
        0,
        0,
        at(0),
        at(0),
    )
    .unwrap()
}

fn scopes() -> AccessScopeSet {
    AccessScopeSet::try_new(&[ScopeKey::IdentityRead, ScopeKey::IdentityWrite]).unwrap()
}

fn raw_digest() -> Sha256Digest {
    // Independent standard SHA-256 vector for 32 zero bytes, with no PAT tag.
    Sha256Digest::parse("sha256:66687aadf862bd776c8fc18b8e9f8e20089714856ee233b3902a591d0d5f2925")
        .unwrap()
}

fn client_fixture() -> (
    ApplicationClient,
    RegisteredClientCredential,
    AccessConsentRevision,
) {
    let owner = owner();
    let client = ApplicationClient::register(
        ClientId::new_v7(),
        WorkspaceId::new_v7(),
        owner.id(),
        AccessCredentialName::try_new("Synthetic CLI").unwrap(),
        ApplicationClientPurpose::Cli,
        at(0),
    )
    .unwrap();
    let credential = RegisteredClientCredential::issue(
        CredentialId::new_v7(),
        &client,
        raw_digest(),
        at(0),
        at(86_400),
    )
    .unwrap();
    let consent = AccessConsentRevision::grant(
        AccessConsentRevisionId::new_v7(),
        &client,
        &owner,
        ProfileId::new_v7(),
        ProfileGrantId::new_v7(),
        scopes().digest(),
        at(0),
    )
    .unwrap();
    (client, credential, consent)
}

#[test]
fn client_results_preserve_exact_models_and_transfer_the_secret_after_registration_or_rotation() {
    let (mut client, mut credential, consent) = client_fixture();
    for rotation in [false, true] {
        if rotation {
            client.advance_credential_epoch().unwrap();
            credential = RegisteredClientCredential::issue(
                CredentialId::new_v7(),
                &client,
                raw_digest(),
                at(10),
                at(86_410),
            )
            .unwrap();
            assert!(consent.created_at() < credential.created_at());
        }
        let outcome = IssuedAccessClientCredential::try_new(
            client.clone(),
            credential.clone(),
            consent.clone(),
            scopes(),
            SecretMaterial::from_bytes([0; 32]),
        )
        .unwrap();
        assert_eq!(outcome.client(), &client);
        assert_eq!(outcome.credential(), &credential);
        assert_eq!(outcome.consent(), &consent);
        assert_eq!(outcome.scopes(), &scopes());
        let transferred = outcome.into_secret();
        assert!(transferred.constant_time_eq(&SecretMaterial::from_bytes([0; 32])));
        assert_eq!(transferred.expose_hex(), "00".repeat(32));
    }
}

#[test]
fn client_results_reject_ineligible_clients_and_credential_binding_or_state_mismatches() {
    for problem in [
        "workspace",
        "client",
        "epoch",
        "revoked credential",
        "no expiry",
        "revoked client",
        "ownerless",
        "node",
        "device",
    ] {
        let (mut client, original, consent) = client_fixture();
        let mut workspace = original.workspace_id();
        let mut client_id = original.client_id();
        let mut epoch = original.epoch();
        let mut expiry = original.expires_at();
        let mut revoked = None;
        match problem {
            "workspace" => workspace = WorkspaceId::new_v7(),
            "client" => client_id = ClientId::new_v7(),
            "epoch" => epoch += 1,
            "revoked credential" => revoked = Some(at(0)),
            "no expiry" => expiry = None,
            "revoked client" => {
                client.revoke();
            }
            _ => {
                let classification = match problem {
                    "node" => ApplicationClientClassification::try_from_persisted(
                        ClientAuthenticationType::FirstParty,
                        ApplicationClientPurpose::Node,
                    )
                    .unwrap(),
                    "device" => ApplicationClientClassification::try_from_persisted(
                        ClientAuthenticationType::Confidential,
                        ApplicationClientPurpose::Device,
                    )
                    .unwrap(),
                    _ => client.classification(),
                };
                let owner = if problem == "ownerless" {
                    None
                } else {
                    client.owner_subject_id()
                };
                client = ApplicationClient::try_from_persisted(
                    client.id(),
                    client.workspace_id(),
                    owner,
                    client.name().cloned(),
                    classification,
                    client.lifecycle(),
                    client.current_credential_epoch(),
                    client.created_at(),
                )
                .unwrap();
            }
        }
        let credential = RegisteredClientCredential::try_from_persisted(
            original.id(),
            workspace,
            client_id,
            original.digest().clone(),
            epoch,
            original.created_at(),
            expiry,
            revoked,
        )
        .unwrap();
        assert_eq!(
            IssuedAccessClientCredential::try_new(
                client,
                credential,
                consent,
                scopes(),
                SecretMaterial::from_bytes([0; 32])
            )
            .err(),
            Some(AccessIssuanceResultError::InvalidClientCredential),
            "{problem}"
        );
    }
}

#[test]
fn client_results_reject_wrong_consent_binding_time_or_scope_evidence() {
    for problem in [
        "workspace",
        "client",
        "owner",
        "future",
        "digest",
        "revoked",
    ] {
        let (client, credential, original) = client_fixture();
        let mut workspace = original.workspace_id();
        let mut client_id = original.client_id();
        let mut owner = original.subject_id();
        let mut created = original.created_at();
        let mut decision = original.decision().clone();
        let mut revision = 1;
        let mut previous = None;
        match problem {
            "workspace" => workspace = WorkspaceId::new_v7(),
            "client" => client_id = ClientId::new_v7(),
            "owner" => owner = AuthSubjectId::new_v7(),
            "future" => created = at(1),
            "digest" => {
                decision = AccessConsentDecision::Granted(Sha256Digest::from_bytes(&[9; 32]))
            }
            "revoked" => {
                decision = AccessConsentDecision::Revoked;
                revision = 2;
                previous = Some(AccessConsentRevisionId::new_v7());
            }
            _ => unreachable!(),
        }
        let consent = AccessConsentRevision::try_from_persisted(
            original.id(),
            workspace,
            client_id,
            owner,
            original.profile_id(),
            original.profile_grant_id(),
            revision,
            previous,
            decision,
            created,
        )
        .unwrap();
        assert_eq!(
            IssuedAccessClientCredential::try_new(
                client,
                credential,
                consent,
                scopes(),
                SecretMaterial::from_bytes([0; 32])
            )
            .err(),
            Some(AccessIssuanceResultError::InvalidConsent),
            "{problem}"
        );
    }
}

#[test]
fn consent_time_is_between_client_creation_and_credential_issuance_inclusively() {
    let (mut client, _, original) = client_fixture();
    client.advance_credential_epoch().unwrap();
    let credential = RegisteredClientCredential::issue(
        CredentialId::new_v7(),
        &client,
        raw_digest(),
        at(10),
        at(86_410),
    )
    .unwrap();
    for (created, valid) in [(-1, false), (0, true), (5, true), (10, true), (11, false)] {
        let consent = AccessConsentRevision::try_from_persisted(
            original.id(),
            original.workspace_id(),
            original.client_id(),
            original.subject_id(),
            original.profile_id(),
            original.profile_grant_id(),
            original.revision(),
            original.previous_revision_id(),
            original.decision().clone(),
            at(created),
        )
        .unwrap();
        let result = IssuedAccessClientCredential::try_new(
            client.clone(),
            credential.clone(),
            consent,
            scopes(),
            SecretMaterial::from_bytes([0; 32]),
        );
        assert_eq!(result.is_ok(), valid, "consent created at {created}");
        if !valid {
            assert_eq!(
                result.err(),
                Some(AccessIssuanceResultError::InvalidConsent)
            );
        }
    }
}

#[test]
fn client_results_reject_unrelated_plaintext_and_empty_scope_sets() {
    let (client, credential, consent) = client_fixture();
    assert_eq!(
        IssuedAccessClientCredential::try_new(
            client.clone(),
            credential.clone(),
            consent.clone(),
            scopes(),
            SecretMaterial::from_bytes([1; 32])
        )
        .err(),
        Some(AccessIssuanceResultError::SecretMismatch)
    );
    assert_eq!(
        IssuedAccessClientCredential::try_new(
            client,
            credential,
            consent,
            AccessScopeSet::try_new(&[]).unwrap(),
            SecretMaterial::from_bytes([0; 32])
        )
        .err(),
        Some(AccessIssuanceResultError::EmptyScopes)
    );
}

fn pat_secret(byte: u8) -> PersonalAccessTokenSecret {
    PersonalAccessTokenSecret::from_secret(SecretMaterial::from_bytes([byte; 32]))
}

fn pat_fixture() -> PersonalAccessToken {
    let owner = owner();
    let installation = TrailBaseInstallation::try_from_persisted(
        TrailBaseInstanceId::new_v7(),
        Sha256Digest::from_bytes(&[1; 32]),
        Some(Sha256Digest::from_bytes(&[2; 32])),
        TrailBaseActivationState::Active,
        1,
        at(0),
        at(0),
    )
    .unwrap();
    PersonalAccessToken::issue(
        PersonalAccessTokenId::new_v7(),
        WorkspaceId::new_v7(),
        ProfileGrantId::new_v7(),
        AccessCredentialName::try_new("Synthetic PAT").unwrap(),
        pat_secret(0).digest(),
        &owner,
        &installation,
        at(0),
        at(86_400),
    )
    .unwrap()
}

#[test]
fn pat_result_retains_metadata_and_transfers_its_distinct_bearer_secret() {
    let token = pat_fixture();
    let outcome =
        IssuedPersonalAccessToken::try_new(token.clone(), scopes(), pat_secret(0)).unwrap();
    assert_eq!(outcome.token(), &token);
    assert_eq!(outcome.scopes(), &scopes());
    let transferred = outcome.into_secret();
    assert_eq!(transferred.digest(), *token.digest());
    let bearer = transferred.expose_bearer();
    assert_eq!(bearer.as_str(), format!("fasti_pat_{}", "00".repeat(32)));
    assert!(SecretMaterial::try_from_hex(&bearer).is_err());
    assert_eq!(
        PersonalAccessTokenSecret::try_from_bearer(&bearer)
            .unwrap()
            .digest(),
        *token.digest()
    );
}

#[test]
fn pat_results_reject_wrong_secrets_empty_scopes_and_nonfresh_models() {
    let original = pat_fixture();
    assert_eq!(
        IssuedPersonalAccessToken::try_new(original.clone(), scopes(), pat_secret(1)).err(),
        Some(AccessIssuanceResultError::SecretMismatch)
    );
    assert_eq!(
        IssuedPersonalAccessToken::try_new(
            original.clone(),
            AccessScopeSet::try_new(&[]).unwrap(),
            pat_secret(0)
        )
        .err(),
        Some(AccessIssuanceResultError::EmptyScopes)
    );
    for (last_used, revoked, replaced) in [
        (Some(at(0)), None, None),
        (None, Some(at(0)), None),
        (None, Some(at(1)), Some(PersonalAccessTokenId::new_v7())),
    ] {
        let token = PersonalAccessToken::try_from_persisted(
            original.id(),
            original.workspace_id(),
            original.subject_id(),
            original.profile_grant_id(),
            original.name().clone(),
            original.digest().clone(),
            original.auth_epoch(),
            original.authorization_epoch(),
            original.trailbase_instance_id(),
            original.activation_generation(),
            original.created_at(),
            original.expires_at(),
            last_used,
            revoked,
            replaced,
        )
        .unwrap();
        assert_eq!(
            IssuedPersonalAccessToken::try_new(token, scopes(), pat_secret(0)).err(),
            Some(AccessIssuanceResultError::NonfreshPersonalToken)
        );
    }
}
