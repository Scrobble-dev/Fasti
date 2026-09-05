use super::*;
use fasti_application::PortabilityLimits;
use fasti_contracts::{
    CanonicalWorkspaceManifestProjection, ChecksummedWorkspaceManifestDto,
    VerifiedInboundWorkspaceManifest,
};
use std::num::NonZeroU64;
use zeroize::Zeroizing;

// Public interoperability vectors, not credentials. RFC 8032 section 7.1,
// https://www.rfc-editor.org/rfc/rfc8032.txt (IETF Trust, 2017).
const VECTORS: [(&str, &str, &str, &str); 3] = [
    (
        "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
        "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
        "",
        "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
    ),
    (
        "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
        "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
        "72",
        "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
    ),
    (
        "c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7",
        "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
        "af82",
        "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a",
    ),
];

fn unhex(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn rfc8032_vectors_match_exactly_and_repeat() {
    for (seed, public, message, signature) in VECTORS {
        let seed = Zeroizing::new(unhex(seed));
        let key = SigningKey::from_seed(&seed).unwrap();
        let public = unhex(public);
        let message = unhex(message);
        let expected = unhex(signature);
        assert_eq!(key.public_key().as_slice(), public);
        assert_eq!(key.sign(&message).unwrap().as_slice(), expected);
        assert_eq!(key.sign(&message).unwrap().as_slice(), expected);
        assert_eq!(verify(&expected, &message, &public), Ok(true));
    }
}

#[test]
fn generated_keys_and_seed_reconstruction_work() {
    let key = SigningKey::generate().unwrap();
    let message = b"disposable signing fixture";
    assert_eq!(
        verify(&key.sign(message).unwrap(), message, key.public_key()),
        Ok(true)
    );
    let mut seed = Zeroizing::new([0_u8; 32]);
    libsodium_rs::random::fill_bytes(&mut *seed);
    let first = SigningKey::from_seed(seed.as_slice()).unwrap();
    let second = SigningKey::from_seed(seed.as_slice()).unwrap();
    assert_eq!(first.public_key(), second.public_key());
    assert_eq!(first.sign(message), second.sign(message));
}

#[test]
fn signatures_are_bound_to_key_and_exact_message() {
    let first = SigningKey::from_seed(&[11; 32]).unwrap();
    let second = SigningKey::from_seed(&[12; 32]).unwrap();
    let signature = first.sign(b"fixture").unwrap();
    assert_eq!(
        verify(&signature, b"fixture", second.public_key()),
        Ok(false)
    );
    assert_eq!(
        verify(&signature, b"fixturf", first.public_key()),
        Ok(false)
    );
    for byte in 0..signature.len() {
        let mut changed = signature;
        changed[byte] ^= 1;
        assert_eq!(verify(&changed, b"fixture", first.public_key()), Ok(false));
    }
}

#[test]
fn malformed_lengths_are_rejected_without_secret_errors() {
    for length in [0, 31, 33, 63, 64, 65] {
        assert!(matches!(
            SigningKey::from_seed(&Zeroizing::new(vec![0; length])),
            Err(Error::Length)
        ));
    }
    for length in [0, 1, 31, 32, 63, 65] {
        assert_eq!(verify(&vec![0; length], b"", &[0; 32]), Err(Error::Length));
    }
    for length in [0, 1, 31, 33, 63, 64, 65] {
        assert_eq!(verify(&[0; 64], b"", &vec![0; length]), Err(Error::Length));
    }
    assert_eq!(format!("{:?}", Error::Length), "Length");
}

#[test]
fn same_length_malformed_keys_and_signatures_fail() {
    let key = SigningKey::from_seed(&[11; 32]).unwrap();
    let signature = key.sign(b"fixture").unwrap();
    for public in [[0; 32], [0xff; 32]] {
        assert_eq!(verify(&signature, b"fixture", &public), Ok(false));
    }
    for signature in [[0; 64], [0xff; 64]] {
        assert_eq!(verify(&signature, b"fixture", key.public_key()), Ok(false));
    }
}

#[test]
fn exact_probe_message_ceiling_and_one_over() {
    let key = SigningKey::from_seed(&[13; 32]).unwrap();
    let message = vec![0x42; MAX_MESSAGE_BYTES];
    let signature = key.sign(&message).unwrap();
    assert_eq!(verify(&signature, &message, key.public_key()), Ok(true));
    let too_large = vec![0x42; MAX_MESSAGE_BYTES + 1];
    assert_eq!(key.sign(&too_large), Err(Error::Length));
    assert_eq!(
        verify(&signature, &too_large, key.public_key()),
        Err(Error::Length)
    );
}

fn limits() -> PortabilityLimits {
    let bytes = NonZeroU64::new(1_000_000).unwrap();
    let entries = NonZeroU64::new(64).unwrap();
    let one = NonZeroU64::new(1).unwrap();
    PortabilityLimits {
        max_snapshot_bytes: bytes,
        max_wal_growth_bytes: bytes,
        max_archive_bytes: bytes,
        max_uncompressed_bytes: bytes,
        max_entry_bytes: bytes,
        max_entries: entries,
        max_rows_per_stream: entries,
        max_path_bytes: bytes,
        max_path_depth: entries,
        max_decompression_ratio: entries,
        scratch_ceiling_bytes: bytes,
        cleanup_reserve_bytes: bytes,
        backup_step_pages: one,
        backup_step_millis: one,
    }
}

fn projection() -> CanonicalWorkspaceManifestProjection {
    let fixture =
        include_bytes!("../../../contracts/portability/v1/workspace-manifest.example.json");
    assert!(fixture.len() <= MAX_MESSAGE_BYTES);
    let dto: ChecksummedWorkspaceManifestDto = serde_json::from_slice(fixture).unwrap();
    let verified = dto.try_into_application(limits()).unwrap();
    let projection =
        CanonicalWorkspaceManifestProjection::try_from_application(verified.manifest().clone())
            .unwrap();
    assert!(projection.canonical_json_bytes().len() <= MAX_MESSAGE_BYTES);
    projection
}

#[test]
fn real_canonical_projection_is_the_signed_input() {
    let projected = projection();
    let bytes = projected.canonical_json_bytes();
    let key = SigningKey::generate().unwrap();
    let signature = key.sign(bytes).unwrap();
    assert_eq!(verify(&signature, bytes, key.public_key()), Ok(true));
    let parsed =
        VerifiedInboundWorkspaceManifest::try_from_canonical_json(bytes, limits()).unwrap();
    assert_eq!(parsed.manifest(), projected.application_manifest());
    assert_eq!(parsed.manifest_digest(), projected.manifest_digest());
    assert_eq!(
        verify(
            &signature,
            projected.manifest_digest().as_str().as_bytes(),
            key.public_key()
        ),
        Ok(false)
    );
    let mut changed = bytes.to_vec();
    changed[1] ^= 1;
    assert_eq!(verify(&signature, &changed, key.public_key()), Ok(false));
}

#[test]
fn valid_signatures_do_not_make_noncanonical_manifests_valid() {
    let projected = projection();
    let original = projected.canonical_json_bytes();
    let canonical = std::str::from_utf8(original).unwrap();
    let key = SigningKey::generate().unwrap();
    let original_signature = key.sign(original).unwrap();
    let with_whitespace = format!(" {canonical}").into_bytes();
    let duplicate = canonical
        .replacen("{\"manifest\":", "{\"manifest\":null,\"manifest\":", 1)
        .into_bytes();
    let formatted =
        include_bytes!("../../../contracts/portability/v1/workspace-manifest.example.json")
            .to_vec();
    for alternate in [with_whitespace, duplicate, formatted] {
        assert!(alternate.len() <= MAX_MESSAGE_BYTES);
        assert_ne!(alternate, original);
        assert_eq!(
            verify(&original_signature, &alternate, key.public_key()),
            Ok(false)
        );
        let signature = key.sign(&alternate).unwrap();
        assert_eq!(verify(&signature, &alternate, key.public_key()), Ok(true));
        assert!(
            VerifiedInboundWorkspaceManifest::try_from_canonical_json(&alternate, limits())
                .is_err()
        );
    }
}

#[test]
fn actual_native_identity_and_api_sizes() {
    assert_eq!(libsodium_rs::version::version_string(), "1.0.22");
    assert_eq!(libsodium_rs::version::library_version_major(), 26);
    assert_eq!(libsodium_rs::version::library_version_minor(), 4);
    assert_eq!(
        (
            crypto_sign::SEEDBYTES,
            crypto_sign::PUBLICKEYBYTES,
            crypto_sign::SECRETKEYBYTES,
            crypto_sign::BYTES
        ),
        (32, 32, 64, 64)
    );
}
