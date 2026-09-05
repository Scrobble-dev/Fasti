//! Library rejection boundaries only; no production token parser or verifier.
use super::*;
use openidconnect::{ClaimsVerificationError, SignatureVerificationError};

#[test]
fn malformed_compact_tokens_fail_parsing_before_claims_verification() {
    let wire = sign(claims()).to_string();
    let valid: CoreIdToken = wire.parse().unwrap();
    assert!(valid
        .claims(&verifier(), &Nonce::new("synthetic-nonce".into()))
        .is_ok());
    let parts: Vec<_> = wire.split('.').collect();
    for malformed in [
        String::new(),
        parts[0].to_owned(),
        parts[..2].join("."),
        format!("{wire}.extra"),
    ] {
        assert!(malformed.parse::<CoreIdToken>().is_err());
    }
    for index in 0..3 {
        let mut malformed = parts.clone();
        malformed[index] = "%"; // Not a base64url character, in each compact segment.
        assert!(malformed.join(".").parse::<CoreIdToken>().is_err());
    }
}

#[test]
fn unsigned_none_token_parses_but_signature_verification_rejects_it() {
    let nonce = Nonce::new("synthetic-nonce".into());
    let signed = sign(claims());
    assert!(signed.claims(&verifier(), &nonce).is_ok());
    let wire = signed.to_string();
    let payload = wire.split('.').nth(1).unwrap();
    // Fixed base64url encoding of {"alg":"none"}; preserve the valid claims payload.
    let unsigned: CoreIdToken = format!("eyJhbGciOiJub25lIn0.{payload}.").parse().unwrap();
    assert!(matches!(
        unsigned.claims(&verifier(), &nonce),
        Err(ClaimsVerificationError::SignatureVerification(
            SignatureVerificationError::NoSignature
        ))
    ));
}

#[test]
fn missing_required_claims_fail_parsing_except_audience_checked_by_verifier() {
    let value = serde_json::to_value(claims()).unwrap();
    assert!(serde_json::from_value::<CoreIdTokenClaims>(value.clone()).is_ok());
    // This isolates the library's claims deserializer, before compact/signature validation.
    for field in ["iss", "sub", "exp", "iat"] {
        let mut missing = value.clone();
        assert!(missing.as_object_mut().unwrap().remove(field).is_some());
        assert!(
            serde_json::from_value::<CoreIdTokenClaims>(missing).is_err(),
            "missing {field}"
        );
    }
    let mut missing_audience = value;
    assert!(missing_audience
        .as_object_mut()
        .unwrap()
        .remove("aud")
        .is_some());
    let parsed: CoreIdTokenClaims = serde_json::from_value(missing_audience).unwrap();
    assert!(parsed.audiences().is_empty());
    let nonce = Nonce::new("synthetic-nonce".into());
    assert!(sign(claims()).claims(&verifier(), &nonce).is_ok());
    assert!(matches!(
        sign(parsed).claims(&verifier(), &nonce),
        Err(ClaimsVerificationError::InvalidAudience(_))
    ));
}

#[test]
fn valid_rs384_signature_is_rejected_by_default_rs256_algorithm_policy() {
    let token = CoreIdToken::new(
        claims(),
        &key(),
        CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha384,
        None,
        None,
    )
    .unwrap();
    let nonce = Nonce::new("synthetic-nonce".into());
    let explicitly_allowed =
        verifier().set_allowed_algs([CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha384]);
    assert!(token.claims(&explicitly_allowed, &nonce).is_ok());
    assert!(matches!(
        token.claims(&verifier(), &nonce),
        Err(ClaimsVerificationError::SignatureVerification(
            SignatureVerificationError::DisallowedAlg(_)
        ))
    ));
    // Qualification comparison only; Fasti must not widen its algorithm profile implicitly.
}
