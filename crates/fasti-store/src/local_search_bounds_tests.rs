mod local_search_bounds_tests {
    use super::*;
    use crate::identity::{attach_identifier_tx, register_namespace_tx};
    use fasti_application::MAX_LOCAL_SEARCH_RESPONSE_BYTES;
    use fasti_domain::{ExternalIdentifierClaim, NamespaceDefinition, NamespaceLicencePosture};

    fn identifier_value(record: RecordId, ordinal: usize) -> String {
        let prefix = format!("{record}:{ordinal:08}:");
        format!("{prefix}{}", "x".repeat(256 - prefix.len()))
    }

    fn attach_many(node: &TestNode, record: RecordId, count: usize) {
        let mut connection = node.kernel.inner.connection.lock().unwrap();
        let transaction = connection.transaction().unwrap();
        let definition = NamespaceDefinition::try_new(
            "bounds",
            "Bounded Search fixture",
            [Grain::Film],
            ".+",
            "identity",
            NamespaceLicencePosture::Unknown,
        )
        .unwrap();
        register_namespace_tx(
            &transaction,
            node.access.workspace_id(),
            &definition,
            CapabilityKey::RegisterNamespace,
            RequestCorrelationId::new_v7(),
        )
        .unwrap();
        // Reverse insertion makes SQLite's unsorted stream differ from the
        // canonical public order; completed vectors must still be sorted.
        for ordinal in (0..count).rev() {
            let claim = ExternalIdentifierClaim::try_new(
                "bounds",
                Grain::Film,
                identifier_value(record, ordinal),
            )
            .unwrap();
            assert!(attach_identifier_tx(
                &transaction,
                node.access.workspace_id(),
                record,
                &claim,
                CapabilityKey::AttachIdentifier,
                RequestCorrelationId::new_v7()
            )
            .unwrap()
            .created());
        }
        transaction.commit().unwrap();
    }

    fn identifier_count(node: &TestNode) -> i64 {
        node.kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM external_identifiers", [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    #[test]
    fn local_search_bounds_json_string_charge_matches_serde_for_ascii_and_unicode() {
        let ascii: String = (0u8..=127).map(char::from).collect();
        for value in [
            "",
            ascii.as_str(),
            "東京 🐎 café",
            "\"\\\n\r\t\u{0008}\u{000c}",
            "\u{2028}\u{2029}",
        ] {
            assert_eq!(
                crate::identity::json_string_bytes(value),
                serde_json::to_string(value).unwrap().len(),
                "{value:?}"
            );
        }
        for byte in 0u8..=127 {
            let value = char::from(byte).to_string();
            assert_eq!(
                crate::identity::json_string_bytes(&value),
                serde_json::to_string(&value).unwrap().len(),
                "ASCII {byte}"
            );
        }
    }

    #[test]
    fn local_search_bounds_identifier_charge_matches_real_objects_and_comma_budget() {
        let node = node();
        let record = seed(&node, 1, "needle present")[0];
        attach_many(&node, record, 0);
        let mut connection = node.kernel.inner.connection.lock().unwrap();
        let transaction = connection.transaction().unwrap();
        for value in ["plain", "quoted\"\\identifier", "東京🐎"] {
            let claim = ExternalIdentifierClaim::try_new("bounds", Grain::Film, value).unwrap();
            attach_identifier_tx(
                &transaction,
                node.access.workspace_id(),
                record,
                &claim,
                CapabilityKey::AttachIdentifier,
                RequestCorrelationId::new_v7(),
            )
            .unwrap();
        }
        transaction.commit().unwrap();
        let load = |budget| {
            crate::identity::load_record_identifiers_batch(
                &connection,
                node.access.workspace_id(),
                &[record],
                CAPABILITY,
                RequestCorrelationId::new_v7(),
                Some(budget),
            )
            .unwrap()
        };
        let (identifiers, charged) = load(MAX_LOCAL_SEARCH_RESPONSE_BYTES).unwrap();
        let wire: Vec<_> = identifiers[&record]
            .iter()
            .map(|identifier| fasti_contracts::RecordIdentifierDto {
                namespace: identifier.namespace().to_string(),
                grain: identifier.grain().as_str().to_owned(),
                value: identifier.value().to_owned(),
            })
            .collect();
        // Per-object comma allowance plus one byte is exactly the array's
        // separators and brackets. The enclosing Record owns its fixed syntax.
        assert_eq!(charged + 1, serde_json::to_vec(&wire).unwrap().len());
        assert!(load(charged).is_some());
        assert!(
            load(charged - 1).is_none(),
            "no partial identifiers escape at the exact byte boundary"
        );
    }

    #[test]
    fn local_search_bounds_identifier_plan_streams_record_index_without_payload_sort() {
        let node = node();
        let records = seed(&node, 2, "needle present");
        attach_many(&node, records[0], 3);
        attach_many(&node, records[1], 3);
        let connection = node.kernel.inner.connection.lock().unwrap();
        let mut statement = connection
            .prepare(&format!(
                "EXPLAIN QUERY PLAN {}",
                crate::identity::SELECT_RECORD_IDENTIFIERS
            ))
            .unwrap();
        let steps = statement
            .query_map(
                params![
                    node.access.workspace_id().to_string(),
                    serde_json::to_string(&[records[0]]).unwrap()
                ],
                |row| row.get::<_, String>(3),
            )
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert!(
            steps.iter().any(|step| step
                .contains("SEARCH identifier USING INDEX external_identifiers_record_idx")
                && step.contains("record_id=?")),
            "selected Record index must bound hydration: {steps:?}"
        );
        assert!(
            !steps
                .iter()
                .any(|step| step.contains("TEMP B-TREE") || step.contains("SCAN identifier")),
            "no eager identifier payload sort or table scan: {steps:?}"
        );
    }

    #[test]
    fn local_search_bounds_complete_identifiers_continue_even_from_original_final_page() {
        for total in [2, 102] {
            let node = node();
            let mut expected = seed(&node, total, "needle present");
            expected.sort_by_key(ToString::to_string);
            for record in &expected[..2] {
                attach_many(&node, *record, 8000);
            }
            let mut query = request(node.access, "needle present");
            let mut found = Vec::new();
            let mut pages = 0;
            loop {
                let page = node.kernel.search_local_records(&query).unwrap();
                pages += 1;
                if pages == 1 {
                    assert_eq!(
                        page.records.len(),
                        1,
                        "two complete dense Records exceed the budget"
                    );
                    assert_eq!(page.next.as_ref().unwrap().last_record_id, expected[0]);
                }
                for record in page.records {
                    if expected[..2].contains(&record.record_id()) {
                        let actual: Vec<_> = record
                            .identifiers()
                            .iter()
                            .map(|identifier| {
                                assert_eq!(identifier.namespace().as_str(), "bounds");
                                assert_eq!(identifier.grain(), Grain::Film);
                                identifier.value().to_owned()
                            })
                            .collect();
                        let complete: Vec<_> = (0..8000)
                            .map(|ordinal| identifier_value(record.record_id(), ordinal))
                            .collect();
                        assert_eq!(record.identifiers().len(), 8000);
                        assert_eq!(actual, complete, "no evidence was truncated");
                    } else {
                        assert!(record.identifiers().is_empty());
                    }
                    found.push(record.record_id());
                }
                let Some(next) = page.next else { break };
                if let Some(previous) = &query.after {
                    assert!(next.last_record_id.to_string() > previous.last_record_id.to_string());
                }
                query.after = Some(next);
                assert!(pages < 5, "bounded cursor progress");
            }
            assert_eq!(found, expected);
            assert_eq!(
                identifier_count(&node),
                16000,
                "reads do not delete evidence"
            );
        }
    }

    #[test]
    fn local_search_bounds_2500_ascii_identifiers_fit_and_remain_complete() {
        let node = node();
        let record = seed(&node, 1, "needle present")[0];
        attach_many(&node, record, 2500);
        let page = node
            .kernel
            .search_local_records(&request(node.access, "needle present"))
            .unwrap();
        assert_eq!(page.records.len(), 1);
        assert!(page.next.is_none());
        let actual: Vec<_> = page.records[0]
            .identifiers()
            .iter()
            .map(|identifier| identifier.value().to_owned())
            .collect();
        let expected: Vec<_> = (0..2500)
            .map(|ordinal| identifier_value(record, ordinal))
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(identifier_count(&node), 2500);
    }

    #[test]
    fn local_search_bounds_oversized_first_matching_record_fails_without_skipping_or_writing() {
        let node = node();
        let mut records = seed(&node, 2, "needle present");
        records.sort_by_key(ToString::to_string);
        attach_many(&node, records[0], 16000);
        let query = request(node.access, "needle present");
        for _ in 0..2 {
            let error = node
                .kernel
                .search_local_records(&query)
                .err()
                .expect("oversized full Record");
            assert_eq!(error.code(), ProblemCode::CapacityExceeded);
            assert_eq!(identifier_count(&node), 16000);
        }
    }

    #[test]
    fn local_search_bounds_oversized_nonmatching_posting_does_not_block_later_match() {
        let node = node();
        let mut records = seed(&node, 2, "needle present");
        records.sort_by_key(ToString::to_string);
        attach_many(&node, records[0], 16000);
        // The immutable old title leaves a public posting, but the profile's
        // authoritative title no longer matches this query.
        configure(
            &node,
            node.access,
            vec![set(records[0], TITLE_FIELD_KEY, "Different title")],
        );
        let page = node
            .kernel
            .search_local_records(&request(node.access, "needle present"))
            .unwrap();
        assert_eq!(page.records.len(), 1);
        assert_eq!(page.records[0].record_id(), records[1]);
        assert!(page.next.is_none());
        assert_eq!(identifier_count(&node), 16000);
    }

    #[test]
    fn local_search_bounds_escaped_metadata_preserves_complete_fields_and_pagination() {
        let node = node();
        let mut records = seed(&node, 100, "needle present");
        records.sort_by_key(ToString::to_string);
        let escaped = format!("needle {}", "\\\"".repeat((4096 - 7) / 2));
        let fields = [
            TITLE_FIELD_KEY,
            ORIGINAL_TITLE_FIELD_KEY,
            fasti_domain::OVERVIEW_FIELD_KEY,
            fasti_domain::POSTER_FIELD_KEY,
        ];
        for chunk in records.chunks(20) {
            configure(
                &node,
                node.access,
                chunk
                    .iter()
                    .flat_map(|record| fields.iter().map(|field| set(*record, field, &escaped)))
                    .collect(),
            );
        }
        let mut query = request(node.access, "needle");
        let mut found = Vec::new();
        let mut pages = 0;
        loop {
            let page = node.kernel.search_local_records(&query).unwrap();
            pages += 1;
            let wire_records: Vec<_> = page.records.iter().map(|record| {
                for field in [record.title(), record.poster(), record.original_title(), record.overview()] {
                    assert_eq!(field.value(), Some(escaped.as_str()));
                }
                // Match the existing RecordSummary DTO projection; provenance
                // remains hidden by the existing ResolvedField serde owner.
                serde_json::json!({
                    "record_id": record.record_id().to_string(), "grain":"film", "status":"active",
                    "title": record.title(), "poster": record.poster(),
                    "original_title": record.original_title(), "overview": record.overview(),
                    "release_year": record.release_year(), "latest_activity": null,
                })
            }).collect();
            let wire =
                serde_json::to_vec(&serde_json::json!({"records":wire_records,"next":page.next}))
                    .unwrap();
            assert!(wire.len() <= MAX_LOCAL_SEARCH_RESPONSE_BYTES);
            assert!(!page.records.is_empty());
            found.extend(page.records.into_iter().map(|record| record.record_id()));
            let Some(next) = page.next else { break };
            query.after = Some(next);
            assert!(pages < 5);
        }
        assert_eq!(
            pages, 1,
            "complete escaped fields fit the real 4 MiB budget"
        );
        assert_eq!(found, records);
    }
}
