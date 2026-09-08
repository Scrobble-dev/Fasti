mod metadata_native_query_tests {
    use super::*;
    use fasti_application::{ProviderResponseReuse, GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY};

    const CAP: CapabilityKey = CapabilityKey::SearchMetadata;

    fn record(node: &TestNode) -> RecordId {
        node.kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Edition,
            ))
            .unwrap()
            .record_id()
    }

    fn native_claim(record: RecordId, value: &str, locale: &str, ordinal: i64) -> FieldClaim {
        let at = received(100).value() + chrono::Duration::microseconds(ordinal);
        FieldClaim::try_new_provider(
            MetadataClaimId::new_v7(),
            record,
            field_key(GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY),
            value,
            FieldClaimProvenance::try_new(
                MetadataProviderId::try_new(GOOGLE_BOOKS_PROVIDER_ID).unwrap(),
                ns("googlebooks.volume"),
                "42",
                Some(MetadataLocale::try_new(locale).unwrap()),
                None,
                None,
                digest("a"),
            )
            .unwrap(),
            ReceivedAt::from_application_clock(at),
            Some(at + chrono::Duration::seconds(120)),
            FieldClaimStatus::Fresh,
        )
        .unwrap()
    }

    fn write(
        connection: &Connection,
        workspace: WorkspaceId,
        claim: &FieldClaim,
        policy: Option<&str>,
    ) {
        write_field_claim(
            connection,
            workspace,
            claim.record_id().unwrap(),
            claim.field_key().unwrap(),
            claim,
            CAP,
            RequestCorrelationId::new_v7(),
            policy,
        )
        .unwrap();
    }

    fn load(
        connection: &Connection,
        workspace: WorkspaceId,
        ids: &[RecordId],
    ) -> ApplicationResult<HashMap<RecordId, Vec<FieldClaim>>> {
        native::load_publication_observations(
            connection,
            workspace,
            ids,
            CAP,
            RequestCorrelationId::new_v7(),
            received(150).value(),
            None,
        )
    }

    #[test]
    fn native_query_complete_history_keeps_latest_type_and_buried_locale_conflict() {
        for depth in [257_i64, 4097] {
            let node = TestNode::new();
            let selected = [record(&node), record(&node)];
            let mut connection = node.kernel.inner.connection.lock().unwrap();
            let transaction = connection.transaction().unwrap();
            // The first locale lies outside any 256-row newest-payload window.
            write(
                &transaction,
                node.access.workspace_id(),
                &native_claim(selected[1], "BOOK", "en-US", 0),
                None,
            );
            for ordinal in 1..=depth {
                for id in selected {
                    let value = if ordinal == depth { "MAGAZINE" } else { "BOOK" };
                    write(
                        &transaction,
                        node.access.workspace_id(),
                        &native_claim(id, value, "fr-FR", ordinal),
                        None,
                    );
                }
            }
            transaction.commit().unwrap();
            let found = load(&connection, node.access.workspace_id(), &selected).unwrap();
            assert_eq!(found[&selected[0]].len(), 1, "history depth {depth}");
            assert_eq!(found[&selected[0]][0].value(), "MAGAZINE");
            assert_eq!(
                found[&selected[0]][0].fetched_at(),
                received(100).value() + chrono::Duration::microseconds(depth)
            );
            assert!(
                found[&selected[1]].is_empty(),
                "a buried latest en-US BOOK must conflict with fr-FR MAGAZINE at depth {depth}"
            );
            // Single-Record callers use the same selection, not the old LIMIT256 path.
            let single = load_field_claims(
                &connection,
                node.access.workspace_id(),
                selected[0],
                &field_key(GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY),
                CAP,
                RequestCorrelationId::new_v7(),
                received(150).value(),
            )
            .unwrap();
            assert_eq!(single, found[&selected[0]]);
        }
    }

    #[test]
    fn native_query_known_restriction_blocks_newer_null_even_below_payload_cap() {
        let node = TestNode::new();
        let selected = record(&node);
        let connection = node.kernel.inner.connection.lock().unwrap();
        let old = native_claim(selected, "BOOK", "en-US", 0);
        let policy = ProviderResponseCachePolicy::new(
            ProviderResponseReuse::ValidateEveryReuse,
            old.fetched_at(),
            std::time::Duration::ZERO,
            None,
            None,
        )
        .to_canonical_json();
        write(&connection, node.access.workspace_id(), &old, Some(&policy));
        write(
            &connection,
            node.access.workspace_id(),
            &native_claim(selected, "BOOK", "en-US", 1),
            None,
        );
        assert!(
            load(&connection, node.access.workspace_id(), &[selected]).unwrap()[&selected]
                .is_empty()
        );
        let latest = native_claim(selected, "BOOK", "en-US", 2);
        let permitted = ProviderResponseCachePolicy::new(
            ProviderResponseReuse::Reusable,
            latest.fetched_at(),
            std::time::Duration::ZERO,
            None,
            None,
        )
        .to_canonical_json();
        write(
            &connection,
            node.access.workspace_id(),
            &latest,
            Some(&permitted),
        );
        assert_eq!(
            load(&connection, node.access.workspace_id(), &[selected]).unwrap()[&selected],
            vec![latest]
        );
    }

    #[test]
    fn native_query_exact_sparse_ids_exclude_other_workspace_and_reject_selected_corruption() {
        let node = TestNode::new();
        let mut records = [record(&node), record(&node), record(&node)];
        records.sort_by_key(ToString::to_string);
        let connection = node.kernel.inner.connection.lock().unwrap();
        for id in records {
            write(
                &connection,
                node.access.workspace_id(),
                &native_claim(id, "BOOK", "en-US", 0),
                None,
            );
        }
        let other_workspace = WorkspaceId::new_v7();
        connection
            .execute(
                "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                params![
                    other_workspace.to_string(),
                    timestamp(received(100).value())
                ],
            )
            .unwrap();
        let transaction = connection.unchecked_transaction().unwrap();
        let other = crate::identity::insert_record(
            &transaction,
            other_workspace,
            Grain::Edition,
            CAP,
            RequestCorrelationId::new_v7(),
        )
        .unwrap();
        transaction.commit().unwrap();
        write(
            &connection,
            other_workspace,
            &native_claim(other, "MAGAZINE", "en-US", 0),
            None,
        );
        // Explicit hostile persisted-value fixture; restore the production guard
        // before reading, as in metadata_batch's malformed selected-claim test.
        let guard: String = connection.query_row("SELECT sql FROM sqlite_master WHERE type='trigger' AND name='metadata_field_claims_immutable_update'", [], |row| row.get(0)).unwrap();
        connection
            .execute_batch("DROP TRIGGER metadata_field_claims_immutable_update")
            .unwrap();
        connection
            .execute(
                "UPDATE metadata_field_claims SET value='NOVEL' WHERE record_id=?1",
                [records[1].to_string()],
            )
            .unwrap();
        connection.execute_batch(&guard).unwrap();
        let selected = [records[0], records[2], other];
        let found = load(&connection, node.access.workspace_id(), &selected).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[&records[0]][0].value(), "BOOK");
        assert_eq!(found[&records[2]][0].value(), "BOOK");
        assert!(!found.contains_key(&records[1]));
        assert!(!found.contains_key(&other));
        let foreign = load(&connection, other_workspace, &selected).unwrap();
        assert_eq!(foreign.len(), 1);
        assert_eq!(foreign[&other][0].value(), "MAGAZINE");
        assert!(load(&connection, node.access.workspace_id(), &[])
            .unwrap()
            .is_empty());
        let error = load(&connection, node.access.workspace_id(), &[records[1]]).unwrap_err();
        assert_eq!(error.code(), ProblemCode::IntegrityFailed);
    }

    #[test]
    fn native_query_preserves_ordinary_title_null_policy_fallback() {
        let node = TestNode::new();
        let selected = record(&node);
        let connection = node.kernel.inner.connection.lock().unwrap();
        let key = field_key(TITLE_FIELD_KEY);
        let native = native_claim(selected, "BOOK", "en-US", 0);
        let old = FieldClaim::try_new_provider(
            MetadataClaimId::new_v7(),
            selected,
            key.clone(),
            "Older fresh title",
            native.provenance().clone(),
            received(100),
            Some(received(220).value()),
            FieldClaimStatus::Fresh,
        )
        .unwrap();
        let newer = FieldClaim::try_new_provider(
            MetadataClaimId::new_v7(),
            selected,
            key.clone(),
            "Newer stale title",
            native.provenance().clone(),
            received(110),
            None,
            FieldClaimStatus::Stale,
        )
        .unwrap();
        for claim in [&old, &newer] {
            write(&connection, node.access.workspace_id(), claim, None);
        }
        let claims = load_field_claims(
            &connection,
            node.access.workspace_id(),
            selected,
            &key,
            CAP,
            RequestCorrelationId::new_v7(),
            received(150).value(),
        )
        .unwrap();
        assert_eq!(
            claims.len(),
            2,
            "ordinary NULL-policy history must not be preselected"
        );
        let policy = MetadataProjectionPolicy::default_for_profile(node.access.profile_id());
        let resolved =
            resolve_profile_field(None, &claims, &[], &policy, received(150).value()).unwrap();
        assert_eq!(resolved.value(), Some("Older fresh title"));
        assert_eq!(resolved.provenance().unwrap().claim_id(), old.claim_id());
    }

    #[test]
    fn native_query_ten_thousand_records_keeps_a_sparse_hundred_record_selection() {
        let node = TestNode::new();
        let workspace = node.access.workspace_id();
        let mut connection = node.kernel.inner.connection.lock().unwrap();
        let transaction = connection.transaction().unwrap();
        let mut records = Vec::with_capacity(10_000);
        for _ in 0..10_000 {
            let id = crate::identity::insert_record(
                &transaction,
                workspace,
                Grain::Edition,
                CAP,
                RequestCorrelationId::new_v7(),
            )
            .unwrap();
            write(
                &transaction,
                workspace,
                &native_claim(id, "BOOK", "en-US", 0),
                None,
            );
            records.push(id);
        }
        transaction.commit().unwrap();
        records.sort_by_key(ToString::to_string);
        // Exercise the Search inspection-page size, spanning the whole fixture
        // rather than its first 100 Records. This does not change the more
        // general metadata reader's existing input limit.
        let selected: Vec<_> = records.iter().skip(99).step_by(100).copied().collect();
        assert_eq!(selected.len(), 100);
        assert_eq!(selected.last(), records.last());
        let found = load(&connection, workspace, &selected).unwrap();
        assert_eq!(found.len(), selected.len());
        for id in &selected {
            let claims = &found[id];
            assert_eq!(claims.len(), 1);
            assert_eq!(claims[0].record_id(), Some(*id));
            assert_eq!(claims[0].value(), "BOOK");
        }
        assert!(!found.contains_key(&records[0]));
        assert!(!found.contains_key(&records[9_998]));

        // Measure the actual selected-observation SQL's work, not elapsed time.
        // Temporary selected-key scans are allowed; scanning the 10k source
        // Records or claims is not. Existing plan tests pin the payload boundary.
        let ids = selected_record_ids_json(&selected, CAP, RequestCorrelationId::new_v7()).unwrap();
        let mut statement = connection
            .prepare(native::SELECT_PUBLICATION_OBSERVATIONS)
            .unwrap();
        let returned = statement
            .query_map(
                params![
                    workspace.to_string(),
                    ids,
                    GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY
                ],
                |row| row.get::<_, String>(0),
            )
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(
            returned,
            selected.iter().map(ToString::to_string).collect::<Vec<_>>()
        );
        let scans = statement.get_status(rusqlite::StatementStatus::FullscanStep);
        assert!(
            scans < 1_000,
            "100 selected observations must not scan the 10k source dataset: {scans}"
        );
    }

    #[test]
    fn native_query_plan_ranks_narrow_keys_then_hydrates_selected_payloads() {
        let node = TestNode::new();
        let selected = record(&node);
        let connection = node.kernel.inner.connection.lock().unwrap();
        let sql = native::SELECT_PUBLICATION_OBSERVATIONS;
        let ranked = sql
            .split("ranked_keys AS (")
            .nth(1)
            .unwrap()
            .split(")\n    SELECT selected")
            .next()
            .unwrap();
        for payload in [
            "c.value",
            "c.expires_at",
            "response_policy_json",
            "p.evidence_digest",
        ] {
            assert!(!ranked.contains(payload), "ranking must exclude {payload}");
        }
        assert!(
            ranked.contains("DENSE_RANK()"),
            "equal-time observations remain visible"
        );
        let ids =
            selected_record_ids_json(&[selected], CAP, RequestCorrelationId::new_v7()).unwrap();
        let mut statement = connection
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap();
        let plan = statement
            .query_map(
                params![
                    node.access.workspace_id().to_string(),
                    ids,
                    GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY
                ],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        let selected_node = plan
            .iter()
            .find(|(_, _, detail)| detail == "CO-ROUTINE selected")
            .expect("selected-key coroutine")
            .0;
        let inside_selected = |mut ancestor: i64| {
            while ancestor != 0 && ancestor != selected_node {
                ancestor = plan
                    .iter()
                    .find(|(id, _, _)| *id == ancestor)
                    .expect("plan parent")
                    .1;
            }
            ancestor == selected_node
        };
        assert!(
            plan.iter()
                .any(|(_, parent, detail)| inside_selected(*parent)
                    && detail.contains("metadata_claim_provenance_recent_idx")
                    && ["workspace_id=?", "record_id=?", "field_key=?"]
                        .iter()
                        .all(|predicate| detail.contains(predicate))),
            "{plan:?}"
        );
        assert!(
            plan.iter()
                .any(|(_, parent, detail)| !inside_selected(*parent)
                    && detail.contains("SEARCH c USING PRIMARY KEY")
                    && ["record_id=?", "field_key=?", "source=?", "fetched_at=?"]
                        .iter()
                        .all(|predicate| detail.contains(predicate))),
            "payload hydration must use exact selected keys: {plan:?}"
        );
        for (_, parent, detail) in &plan {
            if detail.contains("TEMP B-TREE") {
                assert!(
                    inside_selected(*parent),
                    "wide payloads must not enter an outer sort: {plan:?}"
                );
            }
        }
    }
}
