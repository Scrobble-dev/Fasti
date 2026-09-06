//! Synthetic command structure and policy checks, not runtime authorization.
//! Execution-time policy calls model the required transaction recheck only.

use chrono::{DateTime, TimeDelta, Utc};
use fasti_application::*;
use fasti_domain::{AccessCredentialName, PersonalAccessTokenId, RequestCorrelationId};
use static_assertions::assert_not_impl_any;
use std::{fmt, time::Duration};

assert_not_impl_any!(CreatePersonalAccessTokenCommand:
    fmt::Debug, Clone, Default, serde::Serialize, serde::de::DeserializeOwned,
    From<RequestAccessContext>, From<ApplicationAccessContext>,
    From<AuthenticatedBrowserSession>, From<SecretMaterial>, From<PersonalAccessTokenSecret>);
assert_not_impl_any!(RotatePersonalAccessTokenCommand:
    fmt::Debug, Clone, Default, serde::Serialize, serde::de::DeserializeOwned,
    From<RequestAccessContext>, From<ApplicationAccessContext>,
    From<AuthenticatedBrowserSession>, From<SecretMaterial>, From<PersonalAccessTokenSecret>);
assert_not_impl_any!(RevokePersonalAccessTokenCommand:
    fmt::Debug, Clone, Default, serde::Serialize, serde::de::DeserializeOwned,
    From<RequestAccessContext>, From<ApplicationAccessContext>,
    From<AuthenticatedBrowserSession>, From<SecretMaterial>, From<PersonalAccessTokenSecret>);

fn request(
    correlation: RequestCorrelationId,
    received: DateTime<Utc>,
) -> AccessAdministrationRequest {
    let boundary = BrowserRequestBoundaryPolicy::try_new("http://127.0.0.1:8420", "127.0.0.1:8420")
        .unwrap()
        .validate(Some("http://127.0.0.1:8420"), Some("127.0.0.1:8420"))
        .unwrap();
    AccessAdministrationRequest::new(BrowserSessionMutationCommand::new(
        correlation,
        SecretMaterial::from_bytes([1; 32]),
        SecretMaterial::from_bytes([2; 32]),
        boundary,
        received,
    ))
}

fn create(
    received: DateTime<Utc>,
    expiry: Option<DateTime<Utc>>,
    policy: TokenPolicy,
    scopes: &[ScopeKey],
) -> Result<CreatePersonalAccessTokenCommand, PersonalAccessTokenInputError> {
    CreatePersonalAccessTokenCommand::try_new(
        request(RequestCorrelationId::new_v7(), received),
        AccessCredentialName::try_new("Synthetic PAT").unwrap(),
        AccessScopeSet::try_new(scopes).unwrap(),
        expiry,
        policy,
    )
}

fn rotate(
    received: DateTime<Utc>,
    expiry: Option<DateTime<Utc>>,
    policy: TokenPolicy,
) -> Result<RotatePersonalAccessTokenCommand, PersonalAccessTokenInputError> {
    RotatePersonalAccessTokenCommand::try_new(
        request(RequestCorrelationId::new_v7(), received),
        PersonalAccessTokenId::new_v7(),
        expiry,
        policy,
    )
}

#[test]
fn commands_retain_browser_evidence_and_exact_targets_without_authority_claims() {
    let received = DateTime::<Utc>::UNIX_EPOCH + TimeDelta::milliseconds(750);
    let correlation = RequestCorrelationId::new_v7();
    let token_id = PersonalAccessTokenId::new_v7();
    let name = AccessCredentialName::try_new("  Synthetic PAT  ").unwrap();
    let scopes =
        AccessScopeSet::try_new(&[ScopeKey::IdentityRead, ScopeKey::IdentityWrite]).unwrap();
    let creation = CreatePersonalAccessTokenCommand::try_new(
        request(correlation, received),
        name.clone(),
        scopes.clone(),
        None,
        TokenPolicy::C2,
    )
    .unwrap();
    let rotation = RotatePersonalAccessTokenCommand::try_new(
        request(correlation, received),
        token_id,
        None,
        TokenPolicy::C2,
    )
    .unwrap();
    let revocation =
        RevokePersonalAccessTokenCommand::new(request(correlation, received), token_id);
    for evidence in [creation.request(), rotation.request(), revocation.request()] {
        let browser = evidence.browser_request();
        assert_eq!(browser.correlation_id(), correlation);
        assert_eq!(browser.now(), received);
        assert!(browser
            .session_secret()
            .constant_time_eq(&SecretMaterial::from_bytes([1; 32])));
        assert!(browser
            .csrf_secret()
            .constant_time_eq(&SecretMaterial::from_bytes([2; 32])));
        assert!(!browser
            .session_secret()
            .constant_time_eq(browser.csrf_secret()));
    }
    assert_eq!(creation.name(), &name);
    assert_eq!(creation.name().as_str(), "Synthetic PAT");
    assert_eq!(creation.scopes(), &scopes);
    assert_eq!(
        creation.scopes().scopes(),
        &[ScopeKey::IdentityWrite, ScopeKey::IdentityRead]
    );
    assert_eq!(rotation.token_id(), token_id);
    assert_eq!(revocation.token_id(), token_id);
    assert_eq!(creation.requested_expires_at(), None);
    assert_eq!(rotation.requested_expires_at(), None);
}

#[test]
fn empty_creation_scopes_are_rejected_without_inventing_delegability_checks() {
    let received = DateTime::<Utc>::UNIX_EPOCH;
    assert_eq!(
        create(received, None, TokenPolicy::C2, &[]).err(),
        Some(PersonalAccessTokenInputError::EmptyScopes)
    );
    // This structural command does not decide whether a scope is delegable.
    assert!(create(received, None, TokenPolicy::C2, &[ScopeKey::ClientEnroll]).is_ok());
}

#[test]
fn omitted_expiry_remains_omitted_until_policy_resolution_at_execution_time() {
    let received = DateTime::<Utc>::UNIX_EPOCH;
    let execution = received + TimeDelta::milliseconds(250);
    let creation = create(received, None, TokenPolicy::C2, &[ScopeKey::IdentityRead]).unwrap();
    let rotation = rotate(received, None, TokenPolicy::C2).unwrap();
    for requested in [
        creation.requested_expires_at(),
        rotation.requested_expires_at(),
    ] {
        assert_eq!(requested, None);
        assert_eq!(
            TokenPolicy::C2.pat_expiry(execution, requested),
            Ok(execution + TimeDelta::days(30))
        );
        assert_ne!(
            TokenPolicy::C2.pat_expiry(execution, requested),
            Ok(received + TimeDelta::days(30))
        );
    }
    assert_eq!(creation.request().browser_request().now(), received);
    assert_eq!(rotation.request().browser_request().now(), received);
}

#[test]
fn explicit_expiry_is_preserved_and_must_be_rechecked_at_execution() {
    let received = DateTime::<Utc>::UNIX_EPOCH + TimeDelta::milliseconds(750);
    let tick = TimeDelta::nanoseconds(1);
    let day = TimeDelta::days(1);
    for lifetime in [
        day,
        day + tick,
        day * 2 - TimeDelta::milliseconds(250),
        day * 365,
    ] {
        let expiry = received + lifetime;
        let creation = create(
            received,
            Some(expiry),
            TokenPolicy::C2,
            &[ScopeKey::IdentityRead],
        )
        .unwrap();
        let rotation = rotate(received, Some(expiry), TokenPolicy::C2).unwrap();
        for requested in [
            creation.requested_expires_at(),
            rotation.requested_expires_at(),
        ] {
            assert_eq!(requested, Some(expiry));
            // Exactly one day remains at this execution instant; a later tick
            // crosses the minimum even though request-time validation passed.
            let boundary = expiry - day;
            assert_eq!(TokenPolicy::C2.pat_expiry(boundary, requested), Ok(expiry));
            assert_eq!(
                TokenPolicy::C2.pat_expiry(boundary + tick, requested),
                Err(TokenPolicyInputError)
            );
            assert_eq!(
                TokenPolicy::C2.pat_expiry(expiry, requested),
                Err(TokenPolicyInputError)
            );
        }
    }
    for lifetime in [-tick, TimeDelta::zero(), day - tick, day * 365 + tick] {
        let expiry = Some(received + lifetime);
        assert_eq!(
            create(received, expiry, TokenPolicy::C2, &[ScopeKey::IdentityRead]).err(),
            Some(PersonalAccessTokenInputError::InvalidExpiry)
        );
        assert_eq!(
            rotate(received, expiry, TokenPolicy::C2).err(),
            Some(PersonalAccessTokenInputError::InvalidExpiry)
        );
    }
}

#[test]
fn custom_policy_applies_to_both_commands_and_later_default_resolution() {
    let day = Duration::from_secs(86_400);
    let policy = TokenPolicy::try_new(day * 2, day * 4, day * 7, day, day * 365).unwrap();
    let received = DateTime::<Utc>::UNIX_EPOCH;
    let execution = received + TimeDelta::milliseconds(125);
    let creation = create(received, None, policy, &[ScopeKey::IdentityRead]).unwrap();
    let rotation = rotate(received, None, policy).unwrap();
    for requested in [
        creation.requested_expires_at(),
        rotation.requested_expires_at(),
    ] {
        assert_eq!(requested, None);
        assert_eq!(
            policy.pat_expiry(execution, requested),
            Ok(execution + TimeDelta::days(4))
        );
    }
    for (lifetime, valid) in [
        (TimeDelta::days(2) - TimeDelta::nanoseconds(1), false),
        (TimeDelta::days(2), true),
        (TimeDelta::days(3) + TimeDelta::milliseconds(125), true),
        (TimeDelta::days(7), true),
        (TimeDelta::days(7) + TimeDelta::nanoseconds(1), false),
    ] {
        let requested = Some(received + lifetime);
        assert_eq!(
            create(received, requested, policy, &[ScopeKey::IdentityRead]).is_ok(),
            valid
        );
        assert_eq!(rotate(received, requested, policy).is_ok(), valid);
    }
}

#[test]
fn chrono_overflow_fails_at_request_or_execution_without_resolving_a_stored_default() {
    let max = DateTime::<Utc>::MAX_UTC;
    assert_eq!(
        create(max, None, TokenPolicy::C2, &[ScopeKey::IdentityRead]).err(),
        Some(PersonalAccessTokenInputError::InvalidExpiry)
    );
    assert_eq!(
        rotate(max, None, TokenPolicy::C2).err(),
        Some(PersonalAccessTokenInputError::InvalidExpiry)
    );
    let received = max - TimeDelta::days(31);
    let execution = max - TimeDelta::days(29);
    let creation = create(received, None, TokenPolicy::C2, &[ScopeKey::IdentityRead]).unwrap();
    let rotation = rotate(received, None, TokenPolicy::C2).unwrap();
    for requested in [
        creation.requested_expires_at(),
        rotation.requested_expires_at(),
    ] {
        assert_eq!(requested, None);
        assert_eq!(
            TokenPolicy::C2.pat_expiry(execution, requested),
            Err(TokenPolicyInputError)
        );
    }
    // Explicit representable expiry requires no overflowing default addition.
    let received = max - TimeDelta::days(1);
    assert_eq!(
        create(
            received,
            Some(max),
            TokenPolicy::C2,
            &[ScopeKey::IdentityRead]
        )
        .unwrap()
        .requested_expires_at(),
        Some(max)
    );
    assert_eq!(
        rotate(received, Some(max), TokenPolicy::C2)
            .unwrap()
            .requested_expires_at(),
        Some(max)
    );
}
