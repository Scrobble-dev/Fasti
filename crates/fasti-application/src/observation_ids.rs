//! Deterministic operation ID and evidence digest derivation, shared by every
//! adapter that turns an external observation into an [`AcceptObservationCommand`]
//! (webhook ingest, Nuvio pairing, and any future source).
//!
//! [`AcceptObservationCommand`]: crate::AcceptObservationCommand

use fasti_domain::{OperationId, Sha256Digest};

/// Deterministically derives an operation ID from a source-specific lexeme.
///
/// The same lexeme always yields the same ID, so retries, re-ordering, and
/// crashes replay the existing receipt instead of creating a duplicate.
pub fn derive_deterministic_operation_id(lexeme: &str) -> OperationId {
    let mut state: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in lexeme.as_bytes() {
        state ^= u64::from(*byte);
        state = state.wrapping_mul(0x0000_0100_0000_01b3);
    }

    let mut hex = String::with_capacity(64);
    let mut lane = state;
    for _ in 0..4 {
        use std::fmt::Write;
        let _ = write!(hex, "{lane:016x}");
        lane = lane.wrapping_mul(0x9e37_79b9_7f4a_7c15).rotate_left(31);
    }
    hex.truncate(32);

    let mut nibbles: Vec<char> = hex.chars().collect();
    nibbles[12] = '7';
    nibbles[16] = '8';
    let hex: String = nibbles.into_iter().collect();

    format!("op_{hex}")
        .parse()
        .expect("derived operation id is a valid v7 identifier")
}

/// Deterministically derives an evidence digest from a source-specific lexeme.
pub fn derive_deterministic_evidence_digest(lexeme: &str) -> Sha256Digest {
    let mut state: u64 = 0x8422_2325_cbf2_9ce4;
    for byte in lexeme.as_bytes() {
        state ^= u64::from(*byte);
        state = state.wrapping_mul(0x0000_0100_0000_01b3);
    }

    let mut hex = String::with_capacity(64);
    let mut lane = state;
    for _ in 0..4 {
        use std::fmt::Write;
        let _ = write!(hex, "{lane:016x}");
        lane = lane.wrapping_mul(0x9e37_79b9_7f4a_7c15).rotate_left(31);
    }
    format!("sha256:{hex}")
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
}
