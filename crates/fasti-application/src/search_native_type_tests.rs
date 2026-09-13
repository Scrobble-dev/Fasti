mod search_native_type_tests {
    use super::*;
    use crate::{
        metadata_field_group, provider_candidate_metadata_fields, GoogleBooksPrintType,
        GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY,
    };
    use fasti_domain::{FieldClaimStatus, MetadataFieldGroup, ReceivedAt, TITLE_FIELD_KEY};

    const LEGACY_BOOK_JSON: &str = r#"{"provider":"google-books","provider_id":"42","kind":"book","title":"A film","original_title":null,"release_year":2026,"authors":[],"image_url":null,"overview":"A description."}"#;

    fn book_data() -> SearchCandidateData {
        let mut data = candidate_data();
        data.provider = "google-books".into();
        data.kind = "book".into();
        data.image_url = None;
        data
    }

    #[test]
    fn legacy_absent_native_type_preserves_candidate_json_bytes() {
        let candidate = SearchCandidate::from_json(LEGACY_BOOK_JSON).unwrap();
        assert_eq!(candidate.data().google_books_print_type, None);
        assert_eq!(
            candidate.to_json().unwrap().as_bytes(),
            LEGACY_BOOK_JSON.as_bytes()
        );
        assert_eq!(SearchCandidate::try_new(book_data()).unwrap(), candidate);
        // Other providers retain their existing normalized wire representation too.
        let data = candidate_data();
        let json = serde_json::to_string(&data).unwrap();
        assert!(!json.contains("google_books_print_type"));
        assert_eq!(
            SearchCandidate::from_json(&json)
                .unwrap()
                .to_json()
                .unwrap(),
            json
        );
    }

    #[test]
    fn native_type_values_are_bounded_canonical_and_round_trip() {
        for (value, wire) in [
            (GoogleBooksPrintType::Book, "BOOK"),
            (GoogleBooksPrintType::Magazine, "MAGAZINE"),
            (GoogleBooksPrintType::Unknown, "UNKNOWN"),
        ] {
            assert_eq!(value.as_str(), wire);
            assert_eq!(GoogleBooksPrintType::parse_claim(wire), Some(value));
            assert!(value.as_str().len() <= 8);
            assert_eq!(
                serde_json::to_string(&value).unwrap(),
                format!("\"{wire}\"")
            );
            let mut data = book_data();
            data.google_books_print_type = Some(value);
            let candidate = SearchCandidate::try_new(data).unwrap();
            let json = candidate.to_json().unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&json).unwrap()
                    ["google_books_print_type"],
                wire
            );
            assert_eq!(SearchCandidate::from_json(&json).unwrap(), candidate);
        }
        assert_eq!(
            GoogleBooksPrintType::from_source(Some("BOOK")),
            GoogleBooksPrintType::Book
        );
        assert_eq!(
            GoogleBooksPrintType::from_source(Some("MAGAZINE")),
            GoogleBooksPrintType::Magazine
        );
        assert_eq!(
            GoogleBooksPrintType::from_source(None),
            GoogleBooksPrintType::Unknown
        );
        for source in ["", "NOVEL", "book", " BOOK", "MAGAZINE "] {
            assert_eq!(
                GoogleBooksPrintType::from_source(Some(source)),
                GoogleBooksPrintType::Unknown
            );
            assert_eq!(GoogleBooksPrintType::parse_claim(source), None);
        }
        let oversized = "x".repeat(MAX_SEARCH_CANDIDATE_BYTES + 1);
        assert_eq!(
            GoogleBooksPrintType::from_source(Some(&oversized)),
            GoogleBooksPrintType::Unknown
        );
        assert_eq!(GoogleBooksPrintType::parse_claim(&oversized), None);
    }

    #[test]
    fn normalized_native_type_rejects_unknown_wire_values_and_other_providers() {
        for invalid in [
            serde_json::json!("NOVEL"),
            serde_json::json!("book"),
            serde_json::json!(""),
            serde_json::json!(42),
            serde_json::json!(["BOOK"]),
            serde_json::json!({"value": "BOOK"}),
        ] {
            let mut json = serde_json::to_value(book_data()).unwrap();
            json["google_books_print_type"] = invalid;
            assert!(serde_json::from_value::<SearchCandidateData>(json.clone()).is_err());
            assert!(SearchCandidate::from_json(&json.to_string()).is_err());
        }
        for value in [
            GoogleBooksPrintType::Book,
            GoogleBooksPrintType::Magazine,
            GoogleBooksPrintType::Unknown,
        ] {
            let mut data = candidate_data();
            data.google_books_print_type = Some(value);
            let json = serde_json::to_string(&data).unwrap();
            assert_eq!(
                SearchCandidate::try_new(data).unwrap_err(),
                SearchEvidenceError::InvalidCandidate
            );
            assert_eq!(
                SearchCandidate::from_json(&json).unwrap_err(),
                SearchEvidenceError::InvalidCandidate
            );
        }
    }

    #[test]
    fn native_type_metadata_keeps_the_original_observation_and_lifetime() {
        assert_eq!(GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY, "google_books.print_type");
        let at = lifetime().created_at();
        let digest = Sha256Digest::from_bytes(&[9; 32]);
        let locale = MetadataLocale::try_new("fr-FR").unwrap();
        let region = MetadataRegion::try_new("FR").unwrap();
        for value in [
            GoogleBooksPrintType::Book,
            GoogleBooksPrintType::Magazine,
            GoogleBooksPrintType::Unknown,
        ] {
            for (status, expires_at) in [
                (FieldClaimStatus::Fresh, Some(at + Duration::seconds(120))),
                (FieldClaimStatus::Stale, None),
            ] {
                let mut data = book_data();
                data.google_books_print_type = Some(value);
                let candidate = SearchCandidate::try_new(data).unwrap();
                let fields = provider_candidate_metadata_fields(
                    &candidate,
                    Some(locale.clone()),
                    Some(region.clone()),
                    &digest,
                    ReceivedAt::from_application_clock(at),
                    expires_at,
                    status,
                )
                .unwrap();
                let title = fields
                    .iter()
                    .find(|field| field.field_key().as_str() == TITLE_FIELD_KEY)
                    .unwrap();
                let native: Vec<_> = fields
                    .iter()
                    .filter(|field| field.field_key().as_str() == GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY)
                    .collect();
                assert_eq!(native.len(), 1);
                let field = native[0];
                assert_eq!(
                    metadata_field_group(field.field_key()),
                    Some(MetadataFieldGroup::BasicInfo)
                );
                let claim = field.claim();
                assert_eq!(claim.value(), value.as_str());
                assert!(claim.record_id().is_none());
                assert!(claim.field_key().is_none());
                assert_eq!(claim.fetched_at(), at);
                assert_eq!(claim.expires_at(), expires_at);
                assert_eq!(claim.initial_status(), status);
                assert_eq!(claim.provenance(), title.claim().provenance());
                let provenance = claim.provenance();
                assert!(provenance.is_complete());
                assert_eq!(provenance.provider_id().unwrap().as_str(), "google-books");
                assert_eq!(provenance.source_namespace().as_str(), "googlebooks.volume");
                assert_eq!(provenance.source_identifier(), Some("42"));
                assert_eq!(provenance.locale(), Some(&locale));
                assert_eq!(provenance.region(), Some(&region));
                assert_eq!(provenance.source_version(), None);
                assert_eq!(provenance.evidence_digest(), Some(&digest));
                assert_eq!(
                    claim.status_at(at + Duration::days(1)),
                    FieldClaimStatus::Stale
                );
            }
        }
    }

    #[test]
    fn newly_observed_unknown_is_a_fact_but_legacy_absence_is_not() {
        let at = lifetime().created_at();
        let digest = Sha256Digest::from_bytes(&[10; 32]);
        for source in [None, Some("NEW_UPSTREAM_TYPE")] {
            let mut observed = book_data();
            observed.google_books_print_type = Some(GoogleBooksPrintType::from_source(source));
            let project = |data| {
                provider_candidate_metadata_fields(
                    &SearchCandidate::try_new(data).unwrap(),
                    None,
                    None,
                    &digest,
                    ReceivedAt::from_application_clock(at),
                    Some(at + Duration::seconds(120)),
                    FieldClaimStatus::Fresh,
                )
                .unwrap()
            };
            let legacy = project(book_data());
            let current = project(observed);
            assert!(legacy
                .iter()
                .all(|field| field.field_key().as_str() != GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY));
            assert_eq!(current.len(), legacy.len() + 1);
            let native = current
                .iter()
                .find(|field| field.field_key().as_str() == GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY)
                .unwrap();
            assert_eq!(native.claim().value(), "UNKNOWN");
            assert_eq!(native.claim().fetched_at(), at);
            assert_eq!(native.claim().provenance().evidence_digest(), Some(&digest));
        }
    }
}
