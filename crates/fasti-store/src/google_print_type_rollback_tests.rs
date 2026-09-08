mod google_print_type_rollback_tests {
    use super::*;
    use fasti_application::{GoogleBooksPrintType, SearchCandidateActionPreparation};
    use fasti_domain::Grain;

    // Exact pre-change candidate wire fields and unknown-field policy from
    // 1252e0a6:crates/fasti-application/src/search.rs. This is a test-only
    // compatibility probe, not an alternative production parser.
    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct PreNativeFactCandidateData {
        provider: String,
        provider_id: String,
        kind: String,
        title: String,
        original_title: Option<String>,
        release_year: Option<u16>,
        authors: Vec<String>,
        image_url: Option<String>,
        overview: Option<String>,
    }

    fn durable_rows(node: &TestNode) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
        google_print_type_tests::rows(
            node,
            &[
                "records",
                "external_identifiers",
                "metadata_field_claims",
                "metadata_claims",
                "metadata_claim_provenance",
                "search_action_receipts",
                "workspace_revisions",
            ],
        )
    }

    #[test]
    fn old_candidate_reader_rejects_native_fact_and_internal_discard_preserves_history() {
        for value in [
            GoogleBooksPrintType::Book,
            GoogleBooksPrintType::Magazine,
            GoogleBooksPrintType::Unknown,
        ] {
            let (node, command) = google_print_type_tests::google_fixture(value);
            let snapshot = node
                .kernel
                .read_search_candidate(&command.request)
                .unwrap()
                .unwrap();
            let stored_json: String = node
                .kernel
                .inner
                .connection
                .lock()
                .unwrap()
                .query_row(
                    "SELECT candidate_json FROM search_candidate_receipts WHERE candidate_receipt_id = ?1",
                    [command.request.candidate_receipt_id.to_string()],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                SearchCandidate::from_json(&stored_json).unwrap(),
                *snapshot.receipt.candidate()
            );
            assert_eq!(
                snapshot.receipt.candidate().data().google_books_print_type,
                Some(value)
            );
            let old_error =
                serde_json::from_str::<PreNativeFactCandidateData>(&stored_json).unwrap_err();
            assert!(old_error.to_string().contains("google_books_print_type"));

            let mut legacy_data = snapshot.receipt.candidate().data().clone();
            legacy_data.google_books_print_type = None;
            let legacy_json = SearchCandidate::try_new(legacy_data)
                .unwrap()
                .to_json()
                .unwrap();
            let old = serde_json::from_str::<PreNativeFactCandidateData>(&legacy_json).unwrap();
            assert_eq!(serde_json::to_string(&old).unwrap(), legacy_json);
            assert_eq!(
                SearchCandidate::from_json(&legacy_json)
                    .unwrap()
                    .data()
                    .google_books_print_type,
                None
            );

            // Reconstruct the existing fixture's exact request. The context
            // equality and populated read below prevent an accidental no-op purge.
            let request = SearchPageRequest {
                correlation_id: RequestCorrelationId::new_v7(),
                access: command.request.access.clone(),
                query: SearchProviderQuery::try_new(
                    SearchQuery::try_new("Native type evidence").unwrap(),
                    command.request.provider.clone(),
                    1,
                    None,
                    None,
                    vec![Grain::Edition],
                )
                .unwrap(),
                outbound_policy: command.request.outbound_policy.clone(),
                terms_revision: command.request.terms_revision.clone(),
            };
            assert_eq!(request.query.receipt_context(), snapshot.context);
            let original_page = node
                .kernel
                .read_cached_search_page(&request, true)
                .unwrap()
                .unwrap();
            assert_eq!(original_page.candidates.len(), 1);
            assert_eq!(original_page.candidates[0], snapshot.receipt);

            let mut other_request = request.clone();
            other_request.access = node
                .add_profile_with_scopes(&[fasti_application::ScopeKey::MetadataSearch])
                .into();
            let other_page = commit(
                &node,
                &other_request,
                std::slice::from_ref(snapshot.receipt.candidate()),
            );
            assert_ne!(other_page.candidates[0].id(), snapshot.receipt.id());
            let action_prepared = node
                .kernel
                .prepare_search_candidate_action(&command)
                .unwrap();
            let saved = node
                .kernel
                .commit_search_candidate_action(&command, &action_prepared, None)
                .unwrap();
            let before = durable_rows(&node);
            assert!(before[..6].iter().all(|rows| !rows.is_empty()));

            // Exercise only the already existing authorized internal port.
            // There is no public purge endpoint or automatic downgrade workflow.
            let prepared = node.kernel.prepare_search_page(&request).unwrap();
            node.kernel
                .discard_cached_search_page(&request, &prepared)
                .unwrap();
            for stale in [false, true] {
                assert!(node
                    .kernel
                    .read_cached_search_page(&request, stale)
                    .unwrap()
                    .is_none());
            }
            assert!(node
                .kernel
                .read_search_candidate(&command.request)
                .unwrap()
                .is_none());
            assert_eq!(
                node.kernel
                    .read_cached_search_page(&other_request, false)
                    .unwrap(),
                Some(other_page)
            );
            assert_eq!(durable_rows(&node), before);
            assert_eq!(
                node.kernel
                    .prepare_search_candidate_action(&command)
                    .unwrap(),
                SearchCandidateActionPreparation::Replay(Box::new(saved.clone()))
            );
            assert_eq!(
                node.kernel
                    .commit_search_candidate_action(&command, &action_prepared, None)
                    .unwrap(),
                saved
            );
            assert_eq!(durable_rows(&node), before);
        }
    }
}
