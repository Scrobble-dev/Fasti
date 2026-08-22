use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};
use thiserror::Error;

use crate::EvidenceId;

const SHA256_PREFIX: &str = "sha256:";
const SHA256_HEX_LENGTH: usize = 64;
const SHA256_CANONICAL_LENGTH: usize = SHA256_PREFIX.len() + SHA256_HEX_LENGTH;

/// A canonical SHA-256 content digest.
///
/// The printable representation is always `sha256:` followed by exactly 64
/// lowercase hexadecimal characters. This value validates representation, not
/// content: the streaming boundary remains responsible for proving that the
/// digest and byte length match the evidence bytes it accepted.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Sha256Digest(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum Sha256DigestError {
    #[error("SHA-256 digest must start with `sha256:`")]
    MissingPrefix,
    #[error("SHA-256 digest must contain exactly 64 hexadecimal characters")]
    WrongLength,
    #[error("SHA-256 digest must use lowercase hexadecimal characters")]
    NonCanonicalHex,
}

impl Sha256Digest {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, Sha256DigestError> {
        value.as_ref().parse()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Sha256Digest {
    type Err = Sha256DigestError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let Some(hex) = value.strip_prefix(SHA256_PREFIX) else {
            return Err(Sha256DigestError::MissingPrefix);
        };
        if value.len() != SHA256_CANONICAL_LENGTH || hex.len() != SHA256_HEX_LENGTH {
            return Err(Sha256DigestError::WrongLength);
        }
        if !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(Sha256DigestError::NonCanonicalHex);
        }
        Ok(Self(value.to_owned()))
    }
}

impl Serialize for Sha256Digest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Sha256Digest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// Immutable reference to already accepted evidence.
///
/// A zero byte length is representable deliberately. Whether an empty payload
/// is allowed depends on the accepting capability, not on evidence identity.
/// The stream adapter must verify both `digest` and `byte_length` against the
/// bytes before constructing an application acceptance request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceReference {
    evidence_id: EvidenceId,
    digest: Sha256Digest,
    byte_length: u64,
}

impl EvidenceReference {
    pub fn new(evidence_id: EvidenceId, digest: Sha256Digest, byte_length: u64) -> Self {
        Self {
            evidence_id,
            digest,
            byte_length,
        }
    }

    pub fn evidence_id(&self) -> EvidenceId {
        self.evidence_id
    }

    pub fn digest(&self) -> &Sha256Digest {
        &self.digest
    }

    pub fn byte_length(&self) -> u64 {
        self.byte_length
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn encode_digest(bytes: &[u8; 32]) -> String {
        let mut encoded = String::with_capacity(SHA256_CANONICAL_LENGTH);
        encoded.push_str(SHA256_PREFIX);
        for byte in bytes {
            use std::fmt::Write as _;
            write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
        }
        encoded
    }

    proptest! {
        #[test]
        fn every_sha256_byte_sequence_has_one_canonical_round_trip(bytes in any::<[u8; 32]>()) {
            let encoded = encode_digest(&bytes);
            let digest: Sha256Digest = encoded.parse().expect("canonical digest");

            prop_assert_eq!(digest.as_str(), encoded.as_str());
            prop_assert_eq!(digest.to_string(), encoded);
        }
    }

    #[test]
    fn rejects_wrong_prefix_length_uppercase_and_non_hex() {
        let valid_hex = "0".repeat(SHA256_HEX_LENGTH);
        assert_eq!(
            valid_hex.parse::<Sha256Digest>(),
            Err(Sha256DigestError::MissingPrefix)
        );
        assert_eq!(
            format!("{SHA256_PREFIX}{}", "0".repeat(63)).parse::<Sha256Digest>(),
            Err(Sha256DigestError::WrongLength)
        );
        assert_eq!(
            format!("{SHA256_PREFIX}{}A", "0".repeat(63)).parse::<Sha256Digest>(),
            Err(Sha256DigestError::NonCanonicalHex)
        );
        assert_eq!(
            format!("{SHA256_PREFIX}{}g", "0".repeat(63)).parse::<Sha256Digest>(),
            Err(Sha256DigestError::NonCanonicalHex)
        );
    }

    #[test]
    fn serde_round_trips_canonical_values_and_rejects_noncanonical_values() {
        let digest = Sha256Digest::parse(format!("{SHA256_PREFIX}{}", "ab".repeat(32)))
            .expect("canonical digest");
        let json = serde_json::to_string(&digest).expect("serialize digest");
        assert_eq!(
            serde_json::from_str::<Sha256Digest>(&json).expect("deserialize digest"),
            digest
        );

        let uppercase = format!(r#""{SHA256_PREFIX}{}""#, "AB".repeat(32));
        assert!(serde_json::from_str::<Sha256Digest>(&uppercase).is_err());
    }

    #[test]
    fn evidence_reference_keeps_identity_digest_and_capability_owned_empty_length() {
        let evidence_id = EvidenceId::new_v7();
        let digest = Sha256Digest::parse(format!("{SHA256_PREFIX}{}", "00".repeat(32)))
            .expect("canonical digest");
        let reference = EvidenceReference::new(evidence_id, digest.clone(), 0);

        assert_eq!(reference.evidence_id(), evidence_id);
        assert_eq!(reference.digest(), &digest);
        assert_eq!(reference.byte_length(), 0);
    }

    #[test]
    fn evidence_reference_rejects_unknown_wire_fields() {
        let input = format!(
            r#"{{"evidence_id":"{}","digest":"{SHA256_PREFIX}{}","byte_length":1,"path":"secret"}}"#,
            EvidenceId::new_v7(),
            "00".repeat(32),
        );
        assert!(serde_json::from_str::<EvidenceReference>(&input).is_err());
    }
}
