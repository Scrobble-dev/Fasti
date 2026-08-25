//! Deterministic operation ID and evidence digest derivation, shared by every
//! adapter that turns an external observation into an [`AcceptObservationCommand`]
//! (webhook ingest, Nuvio pairing, and any future source).
//!
//! Both derivations hash the lexeme with real SHA-256, each under its own
//! domain-separation prefix, so the two outputs never share a preimage: an
//! operation ID and an evidence digest derived from the same lexeme give an
//! attacker no relationship to exploit, and the value literally labeled
//! `sha256:` is one.
//!
//! [`AcceptObservationCommand`]: crate::AcceptObservationCommand

use fasti_domain::{OperationId, Sha256Digest};
use sha2::{Digest, Sha256};

const OPERATION_ID_DOMAIN: &[u8] = b"fasti.observation.operation_id.v1\0";
const EVIDENCE_DIGEST_DOMAIN: &[u8] = b"fasti.observation.evidence_digest.v1\0";

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Deterministically derives an operation ID from a source-specific lexeme.
///
/// The same lexeme always yields the same ID, so retries, re-ordering, and
/// crashes replay the existing receipt instead of creating a duplicate.
pub fn derive_deterministic_operation_id(lexeme: &str) -> OperationId {
    let mut hasher = Sha256::new();
    hasher.update(OPERATION_ID_DOMAIN);
    hasher.update(lexeme.as_bytes());
    let digest = hasher.finalize();

    // OperationId requires a v7-shaped UUID (version nibble 7, RFC 4122
    // variant). The hash has neither by construction, so the two nibbles the
    // format actually inspects are overwritten; every other bit stays
    // hash-derived entropy.
    let mut nibbles: Vec<char> = hex_encode(&digest[..16]).chars().collect();
    nibbles[12] = '7';
    nibbles[16] = '8';
    let hex: String = nibbles.into_iter().collect();

    format!("op_{hex}")
        .parse()
        .expect("derived operation id is a valid v7 identifier")
}

/// Deterministically derives an evidence digest from a source-specific lexeme.
pub fn derive_deterministic_evidence_digest(lexeme: &str) -> Sha256Digest {
    let mut hasher = Sha256::new();
    hasher.update(EVIDENCE_DIGEST_DOMAIN);
    hasher.update(lexeme.as_bytes());
    let digest = hasher.finalize();

    format!("sha256:{}", hex_encode(&digest))
        .parse()
        .expect("derived digest is valid sha256 format")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_lexeme_yields_the_same_operation_id_and_digest() {
        assert_eq!(
            derive_deterministic_operation_id("a"),
            derive_deterministic_operation_id("a")
        );
        assert_eq!(
            derive_deterministic_evidence_digest("a"),
            derive_deterministic_evidence_digest("a")
        );
    }

    #[test]
    fn different_lexemes_yield_different_operation_ids_and_digests() {
        assert_ne!(
            derive_deterministic_operation_id("a"),
            derive_deterministic_operation_id("b")
        );
        assert_ne!(
            derive_deterministic_evidence_digest("a"),
            derive_deterministic_evidence_digest("b")
        );
    }

    /// Pins the exact derived values for a fixed lexeme. A changed constant
    /// (domain tag, byte slicing, nibble positions) would silently re-key
    /// every persisted receipt and turn replays back into new commits --
    /// this must fail loudly, not just "look different in review."
    #[test]
    fn derivation_matches_pinned_golden_vectors() {
        let lexeme = "plex:account:42:key:99182:event:media.scrobble:offset:8040000";
        assert_eq!(
            derive_deterministic_operation_id(lexeme).to_string(),
            "op_e9ef4518278373e08a6b0892f3f345fe"
        );
        assert_eq!(
            derive_deterministic_evidence_digest(lexeme).to_string(),
            "sha256:7880280e3aabe7601f7907af5c08fba68e88dbaffe49811096925b0aaded2dc7"
        );
    }

    #[test]
    fn operation_id_and_evidence_digest_never_share_a_preimage() {
        // Same lexeme, different domain-separation prefixes: the two outputs
        // must not be derivable from one another.
        let lexeme = "shared-lexeme";
        let op_id = derive_deterministic_operation_id(lexeme).to_string();
        let digest = derive_deterministic_evidence_digest(lexeme).to_string();
        let op_hex = op_id.strip_prefix("op_").expect("op_ prefix");
        let digest_hex = digest.strip_prefix("sha256:").expect("sha256: prefix");
        assert_ne!(&digest_hex[..32], op_hex);
    }
}
