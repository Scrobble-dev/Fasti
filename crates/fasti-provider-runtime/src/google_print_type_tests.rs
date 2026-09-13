mod google_print_type_tests {
    use super::*;
    use fasti_application::GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY;
    use serde_json::{json, Value};
    use std::time::Duration;

    fn observed_policy() -> ProviderResponseCachePolicy {
        let received_at = chrono::DateTime::parse_from_rfc3339("2026-09-08T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CACHE_CONTROL,
            HeaderValue::from_static("max-age=60, stale-if-error=30"),
        );
        headers.insert(reqwest::header::AGE, HeaderValue::from_static("10"));
        crate::cache_policy::observe(&headers, received_at, Duration::ZERO)
    }

    fn volume(print_type: Option<Value>) -> Value {
        let mut volume = json!({"id": "book-1", "volumeInfo": {"title": "Volume"}});
        if let Some(print_type) = print_type {
            volume["volumeInfo"]["printType"] = print_type;
        }
        volume
    }

    fn assert_fact(candidate: &ProviderCandidate, expected: GoogleBooksPrintType, body: &[u8]) {
        assert_eq!(candidate.google_books_print_type, Some(expected));
        assert_eq!(
            candidate
                .search_evidence()
                .unwrap()
                .data()
                .google_books_print_type,
            Some(expected)
        );
        assert_eq!(candidate.evidence_digest, provider_evidence_digest(body));
        let policy = observed_policy();
        assert_eq!(candidate.response_cache_policy(), Some(&policy));
        let locale = MetadataLocale::try_new("en-US").unwrap();
        let region = fasti_domain::MetadataRegion::try_new("US").unwrap();
        let fields = candidate
            .metadata_fields(Some(locale.clone()), Some(region.clone()))
            .unwrap();
        assert_eq!(fields.len(), 2, "title and one native observation");
        let native = fields
            .iter()
            .find(|field| field.field_key().as_str() == GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY)
            .unwrap()
            .claim();
        let title = fields
            .iter()
            .find(|field| field.field_key().as_str() == fasti_domain::TITLE_FIELD_KEY)
            .unwrap()
            .claim();
        assert_eq!(native.value(), expected.as_str());
        assert_eq!(native.provenance(), title.provenance());
        assert_eq!(
            native.provenance().source_namespace().as_str(),
            "googlebooks.volume"
        );
        assert_eq!(native.provenance().source_identifier(), Some("book-1"));
        assert_eq!(
            native.provenance().provider_id().unwrap().as_str(),
            GOOGLE_BOOKS_PROVIDER
        );
        assert_eq!(
            native.provenance().evidence_digest(),
            Some(&candidate.evidence_digest)
        );
        assert_eq!(native.provenance().locale(), Some(&locale));
        assert_eq!(native.provenance().region(), Some(&region));
        assert_eq!(native.fetched_at(), policy.received_at());
        assert_eq!(native.fetched_at(), title.fetched_at());
        assert_eq!(
            native.expires_at(),
            Some(policy.received_at() + chrono::Duration::seconds(50))
        );
        assert_eq!(native.expires_at(), title.expires_at());
        assert_eq!(native.initial_status(), FieldClaimStatus::Fresh);
        assert_eq!(native.initial_status(), title.initial_status());
    }

    fn assert_search_and_detail(print_type: Option<Value>, expected: GoogleBooksPrintType) {
        let volume = volume(print_type);
        let search_body =
            serde_json::to_vec(&json!({"totalItems": 1, "items": [volume.clone()]})).unwrap();
        let page = parse_google_candidates(&search_body, 1)
            .unwrap()
            .with_response_cache_policy(observed_policy());
        assert_eq!(page.candidates.len(), 1);
        assert_eq!(page.next_page, None);
        assert_eq!(page.evidence_digest, provider_evidence_digest(&search_body));
        assert_fact(&page.candidates[0], expected, &search_body);

        let detail_body = serde_json::to_vec(&volume).unwrap();
        let detail = google_candidate(
            serde_json::from_slice(&detail_body).unwrap(),
            provider_evidence_digest(&detail_body),
        )
        .unwrap();
        let detail = verify_selected_candidate(detail, "book-1", "book")
            .unwrap()
            .with_response_cache_policy(observed_policy());
        assert_fact(&detail, expected, &detail_body);
        assert_eq!(
            page.candidates[0].search_evidence().unwrap(),
            detail.search_evidence().unwrap()
        );
    }

    #[test]
    fn google_search_and_detail_retain_exact_native_print_types() {
        assert_search_and_detail(Some(json!("BOOK")), GoogleBooksPrintType::Book);
        assert_search_and_detail(Some(json!("MAGAZINE")), GoogleBooksPrintType::Magazine);
    }

    #[test]
    fn google_search_and_detail_record_unknown_instead_of_inventing_book() {
        for print_type in [
            None,
            Some(Value::Null),
            Some(json!("UNKNOWN")),
            Some(json!("book")),
            Some(json!("COMIC")),
            Some(json!("")),
            Some(json!(17)),
            Some(json!(true)),
            Some(json!(["BOOK"])),
            Some(json!({"value": "BOOK"})),
        ] {
            assert_search_and_detail(print_type, GoogleBooksPrintType::Unknown);
        }
    }

    #[test]
    fn google_native_fact_changes_evidence_but_not_public_candidate_shape() {
        let mut candidates = Vec::new();
        for print_type in ["BOOK", "MAGAZINE"] {
            let body = serde_json::to_vec(&volume(Some(json!(print_type)))).unwrap();
            let candidate = google_candidate(
                serde_json::from_slice(&body).unwrap(),
                provider_evidence_digest(&body),
            )
            .unwrap();
            assert_eq!(
                serde_json::to_value(&candidate).unwrap(),
                json!({
                    "provider": GOOGLE_BOOKS_PROVIDER,
                    "provider_id": "book-1",
                    "title": "Volume",
                    "original_title": null,
                    "kind": "book",
                    "release_year": null,
                    "authors": [],
                    "image_url": null,
                    "overview": null
                })
            );
            candidates.push(candidate);
        }
        assert_ne!(candidates[0].evidence_digest, candidates[1].evidence_digest);
        assert_ne!(
            candidates[0].search_evidence().unwrap(),
            candidates[1].search_evidence().unwrap()
        );
    }

    #[test]
    fn tmdb_candidates_do_not_acquire_google_native_facts() {
        let body = br#"{"page":1,"total_pages":1,"results":[{"id":42,"media_type":"movie","title":"Film","adult":false,"printType":"BOOK"}]}"#;
        let page = parse_tmdb_candidates(body, 1)
            .unwrap()
            .with_response_cache_policy(observed_policy());
        assert_eq!(page.candidates.len(), 1);
        let candidate = &page.candidates[0];
        assert_eq!(candidate.google_books_print_type, None);
        assert_eq!(
            candidate
                .search_evidence()
                .unwrap()
                .data()
                .google_books_print_type,
            None
        );
        let fields = candidate.metadata_fields(None, None).unwrap();
        assert_eq!(fields.len(), 1);
        assert!(fields
            .iter()
            .all(|field| field.field_key().as_str() != GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY));
    }
}
