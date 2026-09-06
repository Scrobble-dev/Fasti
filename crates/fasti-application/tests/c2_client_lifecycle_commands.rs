//! Synthetic command structure only. Expected values do not establish authority
//! or demonstrate transaction comparisons, race handling, or revocation.

use chrono::{DateTime, TimeDelta, Utc};
use fasti_application::*;
use fasti_domain::{AccessConsentRevisionId, ClientId, RequestCorrelationId};
use static_assertions::assert_not_impl_any;
use std::{fmt, time::Duration};

assert_not_impl_any!(RotateAccessClientSecretCommand:
    fmt::Debug, Clone, Default, serde::Serialize, serde::de::DeserializeOwned,
    From<RequestAccessContext>, From<ApplicationAccessContext>,
    From<AuthenticatedBrowserSession>, From<SecretMaterial>, From<PersonalAccessTokenSecret>);
assert_not_impl_any!(RevokeAccessClientCommand:
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

fn rotate(
    expected_epoch: u64,
    scopes: &[ScopeKey],
    expires: DateTime<Utc>,
    policy: TokenPolicy,
) -> Result<RotateAccessClientSecretCommand, RotateAccessClientSecretInputError> {
    RotateAccessClientSecretCommand::try_new(
        request(RequestCorrelationId::new_v7(), DateTime::UNIX_EPOCH),
        ClientId::new_v7(),
        expected_epoch,
        None,
        AccessScopeSet::try_new(scopes).unwrap(),
        expires,
        policy,
    )
}

#[test]
fn lifecycle_commands_retain_request_target_and_explicit_consent_expectation() {
    let received = DateTime::<Utc>::UNIX_EPOCH + TimeDelta::milliseconds(750);
    let expires = received + TimeDelta::days(2) - TimeDelta::milliseconds(250);
    let correlation = RequestCorrelationId::new_v7();
    let client = ClientId::new_v7();
    let scopes =
        AccessScopeSet::try_new(&[ScopeKey::IdentityRead, ScopeKey::IdentityWrite]).unwrap();
    for expected_consent in [None, Some(AccessConsentRevisionId::new_v7())] {
        let rotation = RotateAccessClientSecretCommand::try_new(
            request(correlation, received),
            client,
            42,
            expected_consent,
            scopes.clone(),
            expires,
            TokenPolicy::C2,
        )
        .unwrap();
        let revocation = RevokeAccessClientCommand::new(request(correlation, received), client);
        for evidence in [rotation.request(), revocation.request()] {
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
        assert_eq!(rotation.client_id(), client);
        assert_eq!(revocation.client_id(), client);
        assert_eq!(rotation.expected_credential_epoch(), 42);
        // None must survive as an explicit absence expectation for the store.
        assert_eq!(rotation.expected_consent_revision(), expected_consent);
        assert_eq!(rotation.scopes(), &scopes);
        assert_eq!(
            rotation.scopes().scopes(),
            &[ScopeKey::IdentityWrite, ScopeKey::IdentityRead]
        );
        assert_eq!(rotation.expires_at(), expires);
    }
}

#[test]
fn expected_epoch_must_admit_one_next_sqlite_epoch_including_historical_zero() {
    let expiry = DateTime::<Utc>::UNIX_EPOCH + TimeDelta::days(1);
    let max = i64::MAX as u64;
    for epoch in [0, 1, max - 1] {
        let command = rotate(epoch, &[ScopeKey::IdentityRead], expiry, TokenPolicy::C2).unwrap();
        assert_eq!(command.expected_credential_epoch(), epoch);
        assert_eq!(command.expected_consent_revision(), None);
    }
    for epoch in [max, max + 1, u64::MAX] {
        assert_eq!(
            rotate(epoch, &[ScopeKey::IdentityRead], expiry, TokenPolicy::C2).err(),
            Some(RotateAccessClientSecretInputError::InvalidEpoch)
        );
    }
}

#[test]
fn rotation_requires_nonempty_scopes_and_precise_expiry_within_policy_bounds() {
    let now = DateTime::<Utc>::UNIX_EPOCH;
    let day = TimeDelta::days(1);
    let tick = TimeDelta::nanoseconds(1);
    assert_eq!(
        rotate(1, &[], now + day, TokenPolicy::C2).err(),
        Some(RotateAccessClientSecretInputError::EmptyScopes)
    );
    for lifetime in [-tick, TimeDelta::zero(), day - tick, day * 365 + tick] {
        assert_eq!(
            rotate(
                1,
                &[ScopeKey::IdentityRead],
                now + lifetime,
                TokenPolicy::C2
            )
            .err(),
            Some(RotateAccessClientSecretInputError::InvalidExpiry)
        );
    }
    for lifetime in [
        day,
        day + tick,
        day * 2 - TimeDelta::milliseconds(250),
        day * 365,
    ] {
        let expiry = now + lifetime;
        let command = rotate(1, &[ScopeKey::IdentityRead], expiry, TokenPolicy::C2).unwrap();
        assert_eq!(command.expires_at(), expiry);
        let execution_boundary = expiry - day;
        assert_eq!(
            TokenPolicy::C2.client_secret_expiry(execution_boundary, command.expires_at()),
            Ok(expiry)
        );
        assert_eq!(
            TokenPolicy::C2.client_secret_expiry(execution_boundary + tick, command.expires_at()),
            Err(TokenPolicyInputError)
        );
    }
    // Structural input cannot decide the current grant or audience policy.
    assert!(rotate(0, &[ScopeKey::ClientEnroll], now + day, TokenPolicy::C2).is_ok());
}

#[test]
fn rotation_uses_the_supplied_custom_policy_without_rounding_expiry() {
    let day = Duration::from_secs(86_400);
    let policy = TokenPolicy::try_new(day, day * 30, day * 365, day * 2, day * 4).unwrap();
    let now = DateTime::<Utc>::UNIX_EPOCH;
    for (lifetime, valid) in [
        (TimeDelta::days(2) - TimeDelta::nanoseconds(1), false),
        (TimeDelta::days(2), true),
        (TimeDelta::days(3) + TimeDelta::milliseconds(125), true),
        (TimeDelta::days(4), true),
        (TimeDelta::days(4) + TimeDelta::nanoseconds(1), false),
    ] {
        let result = rotate(1, &[ScopeKey::IdentityRead], now + lifetime, policy);
        assert_eq!(result.is_ok(), valid);
        if let Ok(command) = result {
            assert_eq!(command.expires_at(), now + lifetime);
        }
    }
}
