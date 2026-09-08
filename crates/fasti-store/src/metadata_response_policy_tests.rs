mod metadata_response_policy_tests {
    use super::*;
    use fasti_application::{
        LocalSearchRequest, ProviderResponseCachePolicy, ProviderResponseReuse,
        SearchPersistencePort,
    };
    use fasti_domain::SearchQuery;
    use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
    use rusqlite::types::Value;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    fn response_policy(reuse: ProviderResponseReuse) -> ProviderResponseCachePolicy {
        ProviderResponseCachePolicy::new(
            reuse,
            received(100).value(),
            Duration::ZERO,
            Some(Duration::from_secs(120)),
            None,
        )
    }

    fn response_provenance(namespace: &str, identifier: &str) -> FieldClaimProvenance {
        FieldClaimProvenance::try_new(
            MetadataProviderId::try_new("tmdb").unwrap(),
            ns(namespace),
            identifier,
            Some(MetadataLocale::try_new("en-US").unwrap()),
            None,
            Some("v3".to_owned()),
            digest("a"),
        )
        .unwrap()
    }

    fn response_field(
        key: &str,
        namespace: &str,
        identifier: &str,
        seconds: i64,
    ) -> ProviderMetadataField {
        let fetched = received(seconds);
        ProviderMetadataField::new(
            field_key(key),
            FieldClaim::try_new_unbound_provider(
                MetadataClaimId::new_v7(),
                "Original response evidence",
                response_provenance(namespace, identifier),
                fetched,
                Some(fetched.value() + chrono::Duration::seconds(120)),
                FieldClaimStatus::Fresh,
            )
            .unwrap(),
        )
    }

    fn response_rating(record: RecordId, identifier: &str) -> RatingClaim {
        RatingClaim::try_new(
            MetadataClaimId::new_v7(),
            record,
            8_750,
            RatingScale::try_new(0, 10_000).unwrap(),
            response_provenance("tmdb.movie", identifier),
            received(100),
            Some(received(220).value()),
            FieldClaimStatus::Fresh,
        )
        .unwrap()
    }

    fn durable_rows(node: &TestNode) -> Vec<Vec<Vec<Value>>> {
        let connection = node.kernel.inner.connection.lock().unwrap();
        [
            "records",
            "external_identifiers",
            "metadata_field_claims",
            "metadata_claims",
            "metadata_claim_provenance",
            "metadata_rating_claims",
            "metadata_profile_field_overrides",
            "metadata_projections",
            "metadata_attributions",
            "metadata_cache_entries",
            "metadata_cache_claims",
            "metadata_refresh_receipts",
            "local_search_grams",
            "workspace_revisions",
        ]
        .into_iter()
        .map(|table| {
            let sql = format!("SELECT * FROM {table}");
            let columns = connection.prepare(&sql).unwrap().column_count();
            let ordering = (1..=columns)
                .map(|column| column.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let mut statement = connection
                .prepare(&format!("{sql} ORDER BY {ordering}"))
                .unwrap();
            statement
                .query_map([], |row| {
                    (0..columns)
                        .map(|index| row.get::<_, Value>(index))
                        .collect()
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        })
        .collect()
    }

    fn observe_mutations<T>(node: &TestNode, operation: impl FnOnce() -> T) -> (T, Vec<String>) {
        let observed = Arc::new(Mutex::new(Vec::new()));
        {
            let connection = node.kernel.inner.connection.lock().unwrap();
            connection.flush_prepared_statement_cache();
            let observed = Arc::clone(&observed);
            connection
                .authorizer(Some(move |context: AuthContext<'_>| {
                    match context.action {
                        AuthAction::Insert { table_name }
                        | AuthAction::Delete { table_name }
                        | AuthAction::Update { table_name, .. } => {
                            observed.lock().unwrap().push(table_name.to_owned());
                        }
                        _ => {}
                    }
                    Authorization::Allow
                }))
                .unwrap();
        }
        let result = operation();
        node.kernel
            .inner
            .connection
            .lock()
            .unwrap()
            .authorizer(None::<fn(AuthContext<'_>) -> Authorization>)
            .unwrap();
        let writes = observed.lock().unwrap().clone();
        (result, writes)
    }

    #[test]
    fn create_and_apply_reject_whole_response_before_any_payload_statement() {
        for apply in [false, true] {
            for invalid in [
                "no_store",
                "later_namespace",
                "later_identifier",
                "later_observation",
                "later_duplicate",
                "later_digest",
                "later_locale",
                "later_provider",
            ] {
                let node = TestNode::new();
                let mapping = provider_identity_mapping(TMDB_PROVIDER_ID, "movie").unwrap();
                register_mapping(&node, mapping);
                let target = apply.then(|| a_record(&node));
                let mut fields = vec![response_field(TITLE_FIELD_KEY, "tmdb.movie", "438631", 100)];
                let mut policy = response_policy(ProviderResponseReuse::Reusable);
                match invalid {
                    "no_store" => policy = response_policy(ProviderResponseReuse::NoStore),
                    "later_namespace" => {
                        fields.push(response_field(OVERVIEW_FIELD_KEY, "other", "438631", 100))
                    }
                    "later_identifier" => fields.push(response_field(
                        OVERVIEW_FIELD_KEY,
                        "tmdb.movie",
                        "other",
                        100,
                    )),
                    "later_observation" => fields.push(response_field(
                        OVERVIEW_FIELD_KEY,
                        "tmdb.movie",
                        "438631",
                        101,
                    )),
                    "later_duplicate" => {
                        fields.push(response_field(TITLE_FIELD_KEY, "tmdb.movie", "438631", 100))
                    }
                    "later_digest" | "later_locale" | "later_provider" => {
                        let provenance = FieldClaimProvenance::try_new(
                            MetadataProviderId::try_new(if invalid == "later_provider" {
                                "google_books"
                            } else {
                                "tmdb"
                            })
                            .unwrap(),
                            ns("tmdb.movie"),
                            "438631",
                            Some(
                                MetadataLocale::try_new(if invalid == "later_locale" {
                                    "fr-FR"
                                } else {
                                    "en-US"
                                })
                                .unwrap(),
                            ),
                            None,
                            Some("v3".to_owned()),
                            digest(if invalid == "later_digest" { "b" } else { "a" }),
                        )
                        .unwrap();
                        fields.push(ProviderMetadataField::new(
                            field_key(OVERVIEW_FIELD_KEY),
                            FieldClaim::try_new_unbound_provider(
                                MetadataClaimId::new_v7(),
                                "Mixed response",
                                provenance,
                                received(100),
                                Some(received(220).value()),
                                FieldClaimStatus::Fresh,
                            )
                            .unwrap(),
                        ));
                    }
                    _ => unreachable!(),
                }
                let before = durable_rows(&node);
                let (result, mutations) = observe_mutations(&node, || {
                    let identifier = mapping.identifier("438631").unwrap();
                    if let Some(record) = target {
                        node.kernel
                            .apply_provider_metadata(ApplyProviderMetadataCommand::new(
                                RequestCorrelationId::new_v7(),
                                node.access,
                                record,
                                identifier,
                                fields,
                                policy,
                            ))
                    } else {
                        node.kernel
                            .create_provider_record(CreateProviderRecordCommand::new(
                                RequestCorrelationId::new_v7(),
                                node.access,
                                mapping.grain(),
                                identifier,
                                fields,
                                policy,
                            ))
                            .map(|_| ())
                    }
                });
                assert!(result.is_err(), "apply={apply}, case={invalid}");
                assert!(
                    mutations.is_empty(),
                    "apply={apply}, case={invalid}: admitted {mutations:?}"
                );
                assert_eq!(durable_rows(&node), before);
            }
        }
    }

    #[test]
    fn accepted_provider_response_persists_one_policy_and_conflicting_retry_cannot_replace_it() {
        let node = TestNode::new();
        let mapping = provider_identity_mapping(TMDB_PROVIDER_ID, "movie").unwrap();
        register_mapping(&node, mapping);
        let fields = vec![
            response_field(TITLE_FIELD_KEY, "tmdb.movie", "438631", 100),
            response_field(OVERVIEW_FIELD_KEY, "tmdb.movie", "438631", 100),
        ];
        let policy = response_policy(ProviderResponseReuse::Reusable);
        let create = |policy| {
            node.kernel
                .create_provider_record(CreateProviderRecordCommand::new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    mapping.grain(),
                    mapping.identifier("438631").unwrap(),
                    fields.clone(),
                    policy,
                ))
        };
        let record = create(policy).unwrap().record_id();
        {
            let connection = node.kernel.inner.connection.lock().unwrap();
            let mut statement = connection.prepare(
                "SELECT registered.claim_id, registered.response_policy_json, provenance.fetched_at FROM metadata_claims registered JOIN metadata_claim_provenance provenance ON provenance.claim_id = registered.claim_id WHERE registered.record_id = ?1 ORDER BY registered.claim_id"
            ).unwrap();
            let rows = statement
                .query_map([record.to_string()], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            assert_eq!(rows.len(), 2);
            for (id, json, fetched) in rows {
                assert!(fields
                    .iter()
                    .any(|field| field.claim().claim_id().to_string() == id));
                assert_eq!(json.as_deref(), Some(policy.to_canonical_json().as_str()));
                assert_eq!(fetched, timestamp(policy.received_at()));
            }
        }
        let before = durable_rows(&node);
        assert_eq!(create(policy).unwrap().record_id(), record);
        assert_eq!(durable_rows(&node), before);
        let conflict =
            create(response_policy(ProviderResponseReuse::ValidateWhenStale)).unwrap_err();
        assert_eq!(conflict.code(), ProblemCode::IntegrityFailed);
        assert_eq!(durable_rows(&node), before);
    }

    #[test]
    fn field_and_rating_policy_equality_preserves_historical_null_and_original_ids() {
        for historical in [false, true] {
            let node = TestNode::new();
            let record = a_record(&node);
            let field = response_field(TITLE_FIELD_KEY, "tmdb.movie", "438631", 100);
            let rating = response_rating(record, "438631");
            let original = response_policy(ProviderResponseReuse::Reusable).to_canonical_json();
            let changed =
                response_policy(ProviderResponseReuse::ValidateWhenStale).to_canonical_json();
            let policy = (!historical).then_some(original.as_str());
            {
                let connection = node.kernel.inner.connection.lock().unwrap();
                for _ in 0..2 {
                    write_field_claim(
                        &connection,
                        node.access.workspace_id(),
                        record,
                        field.field_key(),
                        field.claim(),
                        CapabilityKey::AttachIdentifier,
                        RequestCorrelationId::new_v7(),
                        policy,
                    )
                    .unwrap();
                    write_rating_claim(
                        &connection,
                        node.access.workspace_id(),
                        &rating,
                        CapabilityKey::AttachIdentifier,
                        RequestCorrelationId::new_v7(),
                        policy,
                    )
                    .unwrap();
                }
                let mut statement = connection.prepare("SELECT claim_id, response_policy_json FROM metadata_claims WHERE record_id = ?1 ORDER BY claim_id").unwrap();
                let rows = statement
                    .query_map([record.to_string()], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
                    })
                    .unwrap()
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .unwrap();
                assert_eq!(rows.len(), 2);
                for (id, stored) in rows {
                    assert!(
                        id == field.claim().claim_id().to_string()
                            || id == rating.claim_id().to_string()
                    );
                    assert_eq!(stored.as_deref(), policy);
                }
            }
            let before = durable_rows(&node);
            let connection = node.kernel.inner.connection.lock().unwrap();
            let changed = Some(if historical {
                original.as_str()
            } else {
                changed.as_str()
            });
            assert_eq!(
                write_field_claim(
                    &connection,
                    node.access.workspace_id(),
                    record,
                    field.field_key(),
                    field.claim(),
                    CapabilityKey::AttachIdentifier,
                    RequestCorrelationId::new_v7(),
                    changed
                )
                .unwrap_err()
                .code(),
                ProblemCode::IntegrityFailed
            );
            assert_eq!(
                write_rating_claim(
                    &connection,
                    node.access.workspace_id(),
                    &rating,
                    CapabilityKey::AttachIdentifier,
                    RequestCorrelationId::new_v7(),
                    changed
                )
                .unwrap_err()
                .code(),
                ProblemCode::IntegrityFailed
            );
            drop(connection);
            assert_eq!(durable_rows(&node), before);
        }
    }

    #[test]
    fn refresh_validates_response_policy_and_all_ratings_before_field_payload_admission() {
        for invalid_rating in [false, true] {
            let node = TestNode::new();
            let (record, provider, prepared) = refresh_fixture(&node);
            let state = provider_state(1);
            node.kernel
                .put_provider_capability_state(node.access.workspace_id(), state.clone())
                .unwrap();
            let fields = vec![response_field(TITLE_FIELD_KEY, "tmdb.movie", "438631", 100)];
            let ratings = vec![response_rating(
                record,
                if invalid_rating {
                    "wrong-source"
                } else {
                    "438631"
                },
            )];
            let policy = response_policy(if invalid_rating {
                ProviderResponseReuse::Reusable
            } else {
                ProviderResponseReuse::NoStore
            });
            let command = CommitMetadataRefreshCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                fasti_domain::OperationId::new_v7(),
                digest("c"),
                prepared,
                provider.clone(),
                state,
                fields,
                ratings,
                Vec::new(),
                MetadataAttribution::try_new(
                    provider,
                    "Metadata supplied by TMDB",
                    "https://developer.themoviedb.org/",
                )
                .unwrap(),
                policy,
            );
            let before = durable_rows(&node);
            let (result, mutations) =
                observe_mutations(&node, || node.kernel.authorize_and_commit_refresh(command));
            assert!(result.is_err());
            assert!(
                mutations.is_empty(),
                "invalid_rating={invalid_rating}: admitted {mutations:?}"
            );
            assert_eq!(durable_rows(&node), before);
        }
    }

    #[test]
    fn refresh_stamps_both_claim_families_and_completed_replay_keeps_original_policy() {
        let node = TestNode::new();
        let (record, provider, prepared) = refresh_fixture(&node);
        let state = provider_state(1);
        node.kernel
            .put_provider_capability_state(node.access.workspace_id(), state.clone())
            .unwrap();
        let observed = received(now().timestamp() - 30);
        let field = response_field(
            TITLE_FIELD_KEY,
            "tmdb.movie",
            "438631",
            observed.value().timestamp(),
        );
        let rating = RatingClaim::try_new(
            MetadataClaimId::new_v7(),
            record,
            8_750,
            RatingScale::try_new(0, 10_000).unwrap(),
            response_provenance("tmdb.movie", "438631"),
            observed,
            Some(observed.value() + chrono::Duration::seconds(120)),
            FieldClaimStatus::Fresh,
        )
        .unwrap();
        let operation = fasti_domain::OperationId::new_v7();
        let observed_policy = |reuse| {
            ProviderResponseCachePolicy::new(
                reuse,
                observed.value(),
                Duration::ZERO,
                Some(Duration::from_secs(120)),
                None,
            )
        };
        let policy = observed_policy(ProviderResponseReuse::Reusable);
        let command = |policy| {
            CommitMetadataRefreshCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                operation,
                digest("d"),
                prepared.clone(),
                provider.clone(),
                state.clone(),
                vec![field.clone()],
                vec![rating.clone()],
                Vec::new(),
                MetadataAttribution::try_new(
                    provider.clone(),
                    "Metadata supplied by TMDB",
                    "https://developer.themoviedb.org/",
                )
                .unwrap(),
                policy,
            )
        };
        let first = node
            .kernel
            .authorize_and_commit_refresh(command(policy))
            .unwrap();
        {
            let connection = node.kernel.inner.connection.lock().unwrap();
            for claim in [field.claim().claim_id(), rating.claim_id()] {
                let json: Option<String> = connection
                    .query_row(
                        "SELECT response_policy_json FROM metadata_claims WHERE claim_id = ?1",
                        [claim.to_string()],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(json, Some(policy.to_canonical_json()));
            }
        }
        let before = durable_rows(&node);
        let (replay, mutations) = observe_mutations(&node, || {
            node.kernel
                .authorize_and_commit_refresh(command(observed_policy(
                    ProviderResponseReuse::NoStore,
                )))
        });
        assert_eq!(replay.unwrap(), first);
        assert!(
            mutations.is_empty(),
            "completed replay admitted {mutations:?}"
        );
        assert_eq!(durable_rows(&node), before);
    }

    #[test]
    fn refresh_replay_checks_rating_and_projection_only_policy_evidence() {
        for projection_only in [false, true] {
            let node = TestNode::new();
            let (record, provider, _) = refresh_fixture(&node);
            let observed = received(now().timestamp() - 30);
            let projected_at = ReceivedAt::from_application_clock(
                observed.value() + chrono::Duration::seconds(20),
            );
            let field = FieldClaim::try_new_provider(
                MetadataClaimId::new_v7(),
                record,
                field_key(TITLE_FIELD_KEY),
                "Historical title",
                response_provenance("tmdb.movie", "438631"),
                observed,
                Some(observed.value() + chrono::Duration::seconds(120)),
                FieldClaimStatus::Fresh,
            )
            .unwrap();
            let rating = RatingClaim::try_new(
                MetadataClaimId::new_v7(),
                record,
                8_750,
                RatingScale::try_new(0, 10_000).unwrap(),
                response_provenance("tmdb.movie", "438631"),
                observed,
                Some(observed.value() + chrono::Duration::seconds(120)),
                FieldClaimStatus::Fresh,
            )
            .unwrap();
            let policy = ProviderResponseCachePolicy::new(
                ProviderResponseReuse::Reusable,
                observed.value(),
                Duration::ZERO,
                Some(Duration::from_secs(120)),
                None,
            )
            .to_canonical_json();
            {
                let connection = node.kernel.inner.connection.lock().unwrap();
                write_field_claim(
                    &connection,
                    node.access.workspace_id(),
                    record,
                    field.field_key().unwrap(),
                    &field,
                    CapabilityKey::RefreshMetadataClaims,
                    RequestCorrelationId::new_v7(),
                    Some(&policy),
                )
                .unwrap();
                write_rating_claim(
                    &connection,
                    node.access.workspace_id(),
                    &rating,
                    CapabilityKey::RefreshMetadataClaims,
                    RequestCorrelationId::new_v7(),
                    Some(&policy),
                )
                .unwrap();
            }
            let resolved = resolve_profile_field(
                None,
                std::slice::from_ref(&field),
                &[],
                &MetadataProjectionPolicy::default_for_profile(node.access.profile_id()),
                projected_at.value(),
            )
            .unwrap();
            assert_eq!(resolved.provenance().unwrap().claim_id(), field.claim_id());
            let outcome = RefreshMetadataClaimsOutcome::new(
                Vec::new(),
                if projection_only {
                    Vec::new()
                } else {
                    vec![RatingClaimView::new(
                        rating.clone(),
                        FieldClaimStatus::Fresh,
                    )]
                },
                vec![MetadataProjection::try_new(
                    node.access.profile_id(),
                    record,
                    field_key(TITLE_FIELD_KEY),
                    resolved,
                    projected_at,
                )
                .unwrap()],
                Vec::new(),
                Vec::new(),
            );
            let operation = fasti_domain::OperationId::new_v7();
            let command = |operation| {
                CommitMetadataRefreshReceiptCommand::new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    operation,
                    digest("c"),
                    record,
                    provider.clone(),
                    outcome.clone(),
                )
            };
            let saved = node
                .kernel
                .authorize_and_commit_refresh_receipt(command(operation))
                .unwrap();
            let read = || {
                ReadMetadataRefreshReceiptCommand::new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    operation,
                    digest("c"),
                    record,
                    provider.clone(),
                )
            };
            assert_eq!(
                node.kernel
                    .authorize_and_read_refresh_receipt(read())
                    .unwrap(),
                Some(saved)
            );
            {
                let connection = node.kernel.inner.connection.lock().unwrap();
                let trigger: String = connection.query_row(
                    "SELECT sql FROM sqlite_master WHERE type='trigger' AND name='metadata_claims_immutable_update'",
                    [], |row| row.get(0)).unwrap();
                connection
                    .execute_batch("DROP TRIGGER metadata_claims_immutable_update")
                    .unwrap();
                let changed = ProviderResponseCachePolicy::new(
                    ProviderResponseReuse::Reusable,
                    observed.value() + chrono::Duration::seconds(1),
                    Duration::ZERO,
                    Some(Duration::from_secs(120)),
                    None,
                )
                .to_canonical_json();
                let id = if projection_only {
                    field.claim_id()
                } else {
                    rating.claim_id()
                };
                connection
                    .execute(
                        "UPDATE metadata_claims SET response_policy_json=?1 WHERE claim_id=?2",
                        params![changed, id.to_string()],
                    )
                    .unwrap();
                connection.execute_batch(&trigger).unwrap();
            }
            let before = durable_rows(&node);
            assert_eq!(
                node.kernel
                    .authorize_and_read_refresh_receipt(read())
                    .unwrap_err()
                    .code(),
                ProblemCode::IntegrityFailed
            );
            assert_eq!(
                node.kernel
                    .authorize_and_commit_refresh_receipt(command(operation))
                    .unwrap_err()
                    .code(),
                ProblemCode::IntegrityFailed
            );
            assert_eq!(
                node.kernel
                    .authorize_and_commit_refresh_receipt(command(
                        fasti_domain::OperationId::new_v7()
                    ))
                    .unwrap_err()
                    .code(),
                ProblemCode::IntegrityFailed
            );
            assert_eq!(durable_rows(&node), before);
        }
    }

    #[test]
    fn no_cache_live_refresh_discloses_once_and_blocks_replay_and_older_receipts() {
        for with_older_receipt in [false, true] {
            let node = TestNode::new();
            let (record, provider, prepared) = refresh_fixture(&node);
            let state = provider_state(1);
            node.kernel
                .put_provider_capability_state(node.access.workspace_id(), state.clone())
                .unwrap();
            let observed = received(now().timestamp() - 30);
            let older_operation = fasti_domain::OperationId::new_v7();
            let restricted_operation = fasti_domain::OperationId::new_v7();
            let command = |restricted: bool, operation| {
                let fetched = ReceivedAt::from_application_clock(
                    observed.value() + chrono::Duration::seconds(if restricted { 10 } else { 0 }),
                );
                let expires =
                    (!restricted).then_some(fetched.value() + chrono::Duration::seconds(120));
                let status = if restricted {
                    FieldClaimStatus::Stale
                } else {
                    FieldClaimStatus::Fresh
                };
                let provenance = response_provenance("tmdb.movie", "438631");
                let field = ProviderMetadataField::new(
                    field_key(TITLE_FIELD_KEY),
                    FieldClaim::try_new_unbound_provider(
                        MetadataClaimId::new_v7(),
                        if restricted {
                            "Validated live response"
                        } else {
                            "Earlier reusable response"
                        },
                        provenance.clone(),
                        fetched,
                        expires,
                        status,
                    )
                    .unwrap(),
                );
                let rating = RatingClaim::try_new(
                    MetadataClaimId::new_v7(),
                    record,
                    8_750,
                    RatingScale::try_new(0, 10_000).unwrap(),
                    provenance,
                    fetched,
                    expires,
                    status,
                )
                .unwrap();
                CommitMetadataRefreshCommand::new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    operation,
                    digest("e"),
                    prepared.clone(),
                    provider.clone(),
                    state.clone(),
                    vec![field],
                    vec![rating],
                    Vec::new(),
                    MetadataAttribution::try_new(
                        provider.clone(),
                        "Metadata supplied by TMDB",
                        "https://developer.themoviedb.org/",
                    )
                    .unwrap(),
                    ProviderResponseCachePolicy::new(
                        if restricted {
                            ProviderResponseReuse::ValidateEveryReuse
                        } else {
                            ProviderResponseReuse::Reusable
                        },
                        fetched.value(),
                        Duration::ZERO,
                        Some(Duration::from_secs(120)),
                        None,
                    ),
                )
            };
            let read = |operation| {
                ReadMetadataRefreshReceiptCommand::new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    operation,
                    digest("e"),
                    record,
                    provider.clone(),
                )
            };
            if with_older_receipt {
                let first = node
                    .kernel
                    .authorize_and_commit_refresh(command(false, older_operation))
                    .unwrap();
                assert_eq!(
                    node.kernel
                        .authorize_and_read_refresh_receipt(read(older_operation))
                        .unwrap(),
                    Some(first)
                );
            }
            let live = node
                .kernel
                .authorize_and_commit_refresh(command(true, restricted_operation))
                .expect("a validated live no-cache response may be disclosed once");
            assert_eq!(live.field_claims().len(), 1);
            assert_eq!(
                live.field_claims()[0].claim().value(),
                "Validated live response"
            );
            assert_eq!(live.field_claims()[0].claim().expires_at(), None);
            assert_eq!(live.field_claims()[0].status(), FieldClaimStatus::Stale);
            assert_eq!(live.rating_claims().len(), 1);
            assert_eq!(live.rating_claims()[0].claim().expires_at(), None);
            assert_eq!(live.rating_claims()[0].status(), FieldClaimStatus::Stale);
            {
                let connection = node.kernel.inner.connection.lock().unwrap();
                for claim in [
                    live.field_claims()[0].claim().claim_id(),
                    live.rating_claims()[0].claim().claim_id(),
                ] {
                    let stored: String = connection
                        .query_row(
                            "SELECT response_policy_json FROM metadata_claims WHERE claim_id=?1",
                            [claim.to_string()],
                            |row| row.get(0),
                        )
                        .unwrap();
                    let policy = ProviderResponseCachePolicy::from_canonical_json(&stored).unwrap();
                    assert_eq!(policy.reuse(), ProviderResponseReuse::ValidateEveryReuse);
                    assert_eq!(
                        timestamp(policy.received_at()),
                        timestamp(observed.value() + chrono::Duration::seconds(10))
                    );
                }
            }
            let before = durable_rows(&node);
            let (_, mutations) = observe_mutations(&node, || {
                assert_eq!(
                    node.kernel
                        .authorize_and_read_refresh_receipt(read(restricted_operation))
                        .unwrap_err()
                        .code(),
                    ProblemCode::MetadataClaimStale
                );
                assert_eq!(
                    node.kernel
                        .authorize_and_commit_refresh(command(true, restricted_operation))
                        .unwrap_err()
                        .code(),
                    ProblemCode::MetadataClaimStale
                );
                if with_older_receipt {
                    assert_eq!(
                        node.kernel
                            .authorize_and_read_refresh_receipt(read(older_operation))
                            .unwrap_err()
                            .code(),
                        ProblemCode::MetadataClaimStale
                    );
                    assert_eq!(
                        node.kernel
                            .authorize_and_commit_refresh(command(false, older_operation))
                            .unwrap_err()
                            .code(),
                        ProblemCode::MetadataClaimStale
                    );
                }
            });
            assert!(mutations.is_empty(), "receipt reuse wrote {mutations:?}");
            assert_eq!(
                durable_rows(&node),
                before,
                "denied disclosure preserves durable receipt and policy history"
            );
        }
    }

    #[test]
    fn all_metadata_readers_validate_policy_evidence_even_when_an_override_wins() {
        for corruption in ["unknown_member", "noncanonical", "observation", "duration"] {
            let node = TestNode::new();
            let (record, _, _) = refresh_fixture(&node);
            let field = response_field(TITLE_FIELD_KEY, "tmdb.movie", "438631", 100);
            let rating = response_rating(record, "438631");
            let policy = response_policy(ProviderResponseReuse::Reusable).to_canonical_json();
            {
                let connection = node.kernel.inner.connection.lock().unwrap();
                write_field_claim(
                    &connection,
                    node.access.workspace_id(),
                    record,
                    field.field_key(),
                    field.claim(),
                    CapabilityKey::AttachIdentifier,
                    RequestCorrelationId::new_v7(),
                    Some(&policy),
                )
                .unwrap();
                write_rating_claim(
                    &connection,
                    node.access.workspace_id(),
                    &rating,
                    CapabilityKey::AttachIdentifier,
                    RequestCorrelationId::new_v7(),
                    Some(&policy),
                )
                .unwrap();
                write_profile_field_override(
                    &connection,
                    node.access.workspace_id(),
                    &ProfileFieldOverride::try_new(
                        node.access.profile_id(),
                        record,
                        field_key(TITLE_FIELD_KEY),
                        "User title",
                        received(200),
                    )
                    .unwrap(),
                    CapabilityKey::AttachIdentifier,
                    RequestCorrelationId::new_v7(),
                )
                .unwrap();

                let mut changed: serde_json::Value = serde_json::from_str(&policy).unwrap();
                let changed = match corruption {
                    "unknown_member" => {
                        changed["unapproved"] = serde_json::json!(true);
                        serde_json::to_string(&changed).unwrap()
                    }
                    "noncanonical" => format!(" {policy}"),
                    "observation" => ProviderResponseCachePolicy::new(
                        ProviderResponseReuse::Reusable,
                        received(101).value(),
                        Duration::ZERO,
                        Some(Duration::from_secs(120)),
                        None,
                    )
                    .to_canonical_json(),
                    "duration" => {
                        changed["corrected_initial_age"]["nanos"] =
                            serde_json::json!(1_000_000_000_u64);
                        serde_json::to_string(&changed).unwrap()
                    }
                    _ => unreachable!(),
                };
                // Simulate on-disk corruption in this isolated fixture, then
                // restore the normal immutable trigger before any reader runs.
                let trigger: String = connection.query_row(
                    "SELECT sql FROM sqlite_master WHERE type='trigger' AND name='metadata_claims_immutable_update'",
                    [], |row| row.get(0)).unwrap();
                connection
                    .execute_batch("DROP TRIGGER metadata_claims_immutable_update")
                    .unwrap();
                for id in [field.claim().claim_id(), rating.claim_id()] {
                    connection
                        .execute(
                            "UPDATE metadata_claims SET response_policy_json=?1 WHERE claim_id=?2",
                            params![changed, id.to_string()],
                        )
                        .unwrap();
                }
                connection.execute_batch(&trigger).unwrap();
            }
            let before = durable_rows(&node);
            {
                let connection = node.kernel.inner.connection.lock().unwrap();
                let capability = CapabilityKey::ReadMetadataProjection;
                let id = RequestCorrelationId::new_v7();
                assert!(
                    load_field_claims(
                        &connection,
                        node.access.workspace_id(),
                        record,
                        field.field_key(),
                        capability,
                        id,
                        received(150).value(),
                    )
                    .is_err(),
                    "{corruption}: single"
                );
                assert!(
                    load_rating_claims(
                        &connection,
                        node.access.workspace_id(),
                        record,
                        capability,
                        id,
                        received(150).value(),
                    )
                    .is_err(),
                    "{corruption}: rating"
                );
                let keys = [
                    TITLE_FIELD_KEY,
                    ORIGINAL_TITLE_FIELD_KEY,
                    OVERVIEW_FIELD_KEY,
                    POSTER_FIELD_KEY,
                    RELEASE_YEAR_FIELD_KEY,
                ]
                .map(field_key);
                assert!(
                    load_record_metadata_batch(
                        &connection,
                        node.access.workspace_id(),
                        node.access.profile_id(),
                        &[record],
                        &keys,
                        capability,
                        id
                    )
                    .is_err(),
                    "{corruption}: batch override"
                );
            }
            assert!(
                node.kernel
                    .authorize_and_read_projection(ReadMetadataProjectionQuery::new(
                        RequestCorrelationId::new_v7(),
                        node.access,
                        record,
                        true
                    ))
                    .is_err(),
                "{corruption}: public projection"
            );
            assert_eq!(durable_rows(&node), before);
        }
    }

    #[test]
    fn missing_or_misbound_registry_is_integrity_failure_even_when_override_wins() {
        for corruption in ["missing", "wrong_workspace", "wrong_record", "wrong_kind"] {
            let node = TestNode::new();
            let (record, _, _) = refresh_fixture(&node);
            let field = response_field(TITLE_FIELD_KEY, "tmdb.movie", "438631", 100);
            let rating = response_rating(record, "438631");
            let policy = response_policy(ProviderResponseReuse::Reusable).to_canonical_json();
            let keys = [
                field_key(TITLE_FIELD_KEY),
                field_key(ORIGINAL_TITLE_FIELD_KEY),
                field_key(OVERVIEW_FIELD_KEY),
                field_key(POSTER_FIELD_KEY),
                field_key(RELEASE_YEAR_FIELD_KEY),
            ];
            let capability = CapabilityKey::ReadMetadataProjection;
            let id = RequestCorrelationId::new_v7();
            {
                let connection = node.kernel.inner.connection.lock().unwrap();
                write_field_claim(
                    &connection,
                    node.access.workspace_id(),
                    record,
                    field.field_key(),
                    field.claim(),
                    CapabilityKey::AttachIdentifier,
                    id,
                    Some(&policy),
                )
                .unwrap();
                write_rating_claim(
                    &connection,
                    node.access.workspace_id(),
                    &rating,
                    CapabilityKey::AttachIdentifier,
                    id,
                    Some(&policy),
                )
                .unwrap();
                write_profile_field_override(
                    &connection,
                    node.access.workspace_id(),
                    &ProfileFieldOverride::try_new(
                        node.access.profile_id(),
                        record,
                        field_key(TITLE_FIELD_KEY),
                        "User title",
                        received(200),
                    )
                    .unwrap(),
                    CapabilityKey::AttachIdentifier,
                    id,
                )
                .unwrap();
                let valid = load_record_metadata_batch(
                    &connection,
                    node.access.workspace_id(),
                    node.access.profile_id(),
                    &[record],
                    &keys,
                    capability,
                    id,
                )
                .unwrap();
                assert_eq!(
                    valid
                        .resolve(record, field.field_key(), capability, id)
                        .unwrap()
                        .value(),
                    Some("User title")
                );
                assert_eq!(
                    load_rating_claims(
                        &connection,
                        node.access.workspace_id(),
                        record,
                        capability,
                        id,
                        received(150).value(),
                    )
                    .unwrap()
                    .len(),
                    1
                );

                // Bypass guards only to simulate corrupt disk state in this
                // isolated fixture. Restore every guard before invoking readers.
                let trigger_names = [
                    "metadata_claims_immutable_update",
                    "metadata_claims_immutable_delete",
                    "metadata_claims_scope_update",
                ];
                let triggers =
                    trigger_names
                        .iter()
                        .map(|name| {
                            connection.query_row(
                    "SELECT sql FROM sqlite_master WHERE type='trigger' AND name=?1", [name],
                    |row| row.get::<_, String>(0)).unwrap()
                        })
                        .collect::<Vec<_>>();
                connection
                    .pragma_update(None, "foreign_keys", "OFF")
                    .unwrap();
                for name in trigger_names {
                    connection
                        .execute_batch(&format!("DROP TRIGGER {name}"))
                        .unwrap();
                }
                for claim in [field.claim().claim_id(), rating.claim_id()] {
                    let changed = match corruption {
                        "missing" => connection.execute("DELETE FROM metadata_claims WHERE claim_id=?1", [claim.to_string()]),
                        "wrong_workspace" => connection.execute("UPDATE metadata_claims SET workspace_id=?1 WHERE claim_id=?2",
                            params![WorkspaceId::new_v7().to_string(), claim.to_string()]),
                        "wrong_record" => connection.execute("UPDATE metadata_claims SET record_id=?1 WHERE claim_id=?2",
                            params![RecordId::new_v7().to_string(), claim.to_string()]),
                        "wrong_kind" => connection.execute("UPDATE metadata_claims SET claim_kind=CASE claim_kind WHEN 'field' THEN 'rating' ELSE 'field' END WHERE claim_id=?1", [claim.to_string()]),
                        _ => unreachable!(),
                    }.unwrap();
                    assert_eq!(changed, 1);
                }
                for trigger in triggers {
                    connection.execute_batch(&trigger).unwrap();
                }
                connection
                    .pragma_update(None, "foreign_keys", "ON")
                    .unwrap();
                let foreign_keys: i64 = connection
                    .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
                    .unwrap();
                assert_eq!(foreign_keys, 1);
                // Both payload rows and the user's override survive the
                // deliberate registry damage; missing payload cannot mask it.
                let fields: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM metadata_claim_provenance WHERE claim_id=?1",
                        [field.claim().claim_id().to_string()],
                        |row| row.get(0),
                    )
                    .unwrap();
                let ratings: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM metadata_rating_claims WHERE claim_id=?1",
                        [rating.claim_id().to_string()],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!((fields, ratings), (1, 1));
            }
            let before = durable_rows(&node);
            let (_, mutations) = observe_mutations(&node, || {
                let connection = node.kernel.inner.connection.lock().unwrap();
                assert_eq!(
                    load_field_claims(
                        &connection,
                        node.access.workspace_id(),
                        record,
                        field.field_key(),
                        capability,
                        id,
                        received(150).value(),
                    )
                    .unwrap_err()
                    .code(),
                    ProblemCode::IntegrityFailed,
                    "{corruption}: single"
                );
                assert_eq!(
                    load_rating_claims(
                        &connection,
                        node.access.workspace_id(),
                        record,
                        capability,
                        id,
                        received(150).value(),
                    )
                    .unwrap_err()
                    .code(),
                    ProblemCode::IntegrityFailed,
                    "{corruption}: rating"
                );
                let batch = load_record_metadata_batch(
                    &connection,
                    node.access.workspace_id(),
                    node.access.profile_id(),
                    &[record],
                    &keys,
                    capability,
                    id,
                );
                assert_eq!(
                    batch
                        .err()
                        .expect("corrupt registry must reject the batch")
                        .code(),
                    ProblemCode::IntegrityFailed,
                    "{corruption}: batch"
                );
            });
            assert!(
                mutations.is_empty(),
                "{corruption}: reader wrote {mutations:?}"
            );
            assert_eq!(
                node.kernel
                    .authorize_and_read_projection(ReadMetadataProjectionQuery::new(
                        RequestCorrelationId::new_v7(),
                        node.access,
                        record,
                        true
                    ))
                    .unwrap_err()
                    .code(),
                ProblemCode::IntegrityFailed,
                "{corruption}: authorized projection"
            );
            assert_eq!(durable_rows(&node), before);
        }
    }

    #[test]
    fn newer_restricted_variant_cannot_resurrect_older_complete_evidence() {
        let provenance = response_provenance("tmdb.movie", "438631");
        for mode in [
            ProviderResponseReuse::NoStore,
            ProviderResponseReuse::ValidateEveryReuse,
            ProviderResponseReuse::ValidateWhenStale,
        ] {
            let old = read_evidence(
                &provenance,
                100,
                Some(220),
                Some(ProviderResponseReuse::Reusable),
            );
            let newer = read_evidence(&provenance, 200, Some(320), Some(mode));
            assert_eq!(
                reusable_metadata_evidence(&[old, newer], received(321).value()),
                vec![false, false],
                "newer mode {mode:?}"
            );
        }
        let old = read_evidence(
            &provenance,
            100,
            Some(220),
            Some(ProviderResponseReuse::Reusable),
        );
        let newer = read_evidence(
            &provenance,
            200,
            Some(320),
            Some(ProviderResponseReuse::Reusable),
        );
        assert_eq!(
            reusable_metadata_evidence(&[old, newer], received(250).value()),
            vec![false, true]
        );
    }

    fn read_evidence<'a>(
        provenance: &'a FieldClaimProvenance,
        fetched: i64,
        expires: Option<i64>,
        mode: Option<ProviderResponseReuse>,
    ) -> MetadataReadEvidence<'a> {
        MetadataReadEvidence {
            provenance,
            fetched_at: received(fetched).value(),
            expires_at: expires.map(|value| received(value).value()),
            policy: mode.map(|reuse| {
                ProviderResponseCachePolicy::new(
                    reuse,
                    received(fetched).value(),
                    Duration::ZERO,
                    Some(Duration::from_secs(3600)),
                    None,
                )
            }),
        }
    }

    #[test]
    fn reuse_keeps_original_short_expiry_exclusive_and_zero_freshness_semantics() {
        let provenance = response_provenance("tmdb.movie", "438631");
        let short = read_evidence(
            &provenance,
            100,
            Some(220),
            Some(ProviderResponseReuse::ValidateWhenStale),
        );
        assert_eq!(
            reusable_metadata_evidence(
                std::slice::from_ref(&short),
                received(220).value() - chrono::Duration::microseconds(1)
            ),
            vec![true]
        );
        assert_eq!(
            reusable_metadata_evidence(&[short], received(220).value()),
            vec![false]
        );
        // A Stale claim with no expiry represents zero original freshness.
        // It cannot satisfy must-revalidate, but reusable stale evidence keeps
        // its separately governed retention window.
        for (mode, allowed) in [
            (ProviderResponseReuse::ValidateWhenStale, false),
            (ProviderResponseReuse::ValidateEveryReuse, false),
            (ProviderResponseReuse::NoStore, false),
            (ProviderResponseReuse::Reusable, true),
        ] {
            assert_eq!(
                reusable_metadata_evidence(
                    &[read_evidence(&provenance, 100, None, Some(mode))],
                    received(150).value()
                ),
                vec![allowed],
                "zero-freshness {mode:?}"
            );
        }
        for mode in [
            ProviderResponseReuse::Reusable,
            ProviderResponseReuse::ValidateWhenStale,
        ] {
            let evidence = read_evidence(&provenance, 100, Some(220), Some(mode));
            assert_eq!(
                reusable_metadata_evidence(
                    std::slice::from_ref(&evidence),
                    received(100).value() - chrono::Duration::nanoseconds(1)
                ),
                vec![false],
                "backward clock {mode:?}"
            );
            assert_eq!(
                reusable_metadata_evidence(&[evidence], received(100).value()),
                vec![true]
            );
        }
    }

    #[test]
    fn equal_time_restrictive_observations_win_independent_of_rating_order() {
        let first = response_provenance("tmdb.movie", "438631");
        let other_response = FieldClaimProvenance::try_new(
            MetadataProviderId::try_new("tmdb").unwrap(),
            ns("tmdb.movie"),
            "438631",
            Some(MetadataLocale::try_new("en-US").unwrap()),
            None,
            Some("another-response".into()),
            digest("b"),
        )
        .unwrap();
        for reverse in [false, true] {
            let mut observations = vec![
                read_evidence(
                    &first,
                    100,
                    Some(220),
                    Some(ProviderResponseReuse::Reusable),
                ),
                read_evidence(
                    &other_response,
                    100,
                    Some(220),
                    Some(ProviderResponseReuse::ValidateEveryReuse),
                ),
            ];
            if reverse {
                observations.reverse();
            }
            assert_eq!(
                reusable_metadata_evidence(&observations, received(150).value()),
                vec![false, false]
            );
        }
    }

    #[test]
    fn reuse_variant_keys_preserve_independent_provider_source_locale_and_region() {
        let variant = |provider: &str,
                       namespace: &str,
                       source: &str,
                       locale: Option<&str>,
                       region: Option<&str>| {
            FieldClaimProvenance::try_new(
                MetadataProviderId::try_new(provider).unwrap(),
                ns(namespace),
                source,
                locale.map(|value| MetadataLocale::try_new(value).unwrap()),
                region.map(|value| MetadataRegion::try_new(value).unwrap()),
                Some("v3".into()),
                digest("a"),
            )
            .unwrap()
        };
        let restricted = variant("tmdb", "tmdb.movie", "438631", Some("en-US"), Some("IE"));
        let independent = [
            variant("other", "tmdb.movie", "438631", Some("en-US"), Some("IE")),
            variant("tmdb", "tmdb.tv", "438631", Some("en-US"), Some("IE")),
            variant("tmdb", "tmdb.movie", "other", Some("en-US"), Some("IE")),
            variant("tmdb", "tmdb.movie", "438631", Some("fr-FR"), Some("IE")),
            variant("tmdb", "tmdb.movie", "438631", Some("en-US"), Some("US")),
            variant("tmdb", "tmdb.movie", "438631", None, Some("IE")),
            variant("tmdb", "tmdb.movie", "438631", Some("en-US"), None),
        ];
        let mut evidence = vec![
            read_evidence(
                &restricted,
                100,
                Some(220),
                Some(ProviderResponseReuse::Reusable),
            ),
            read_evidence(
                &restricted,
                200,
                Some(320),
                Some(ProviderResponseReuse::ValidateEveryReuse),
            ),
        ];
        evidence.extend(independent.iter().map(|provenance| {
            read_evidence(
                provenance,
                100,
                Some(220),
                Some(ProviderResponseReuse::Reusable),
            )
        }));
        assert_eq!(
            reusable_metadata_evidence(&evidence, received(250).value()),
            vec![false, false, true, true, true, true, true, true, true]
        );
    }

    #[test]
    fn historical_unknown_policy_cannot_cancel_known_restrictions_or_prove_wildcard_independence() {
        let complete = response_provenance("tmdb.movie", "438631");
        let wildcard = FieldClaimProvenance::legacy(ns("tmdb.movie"), None);
        let matching_locale = FieldClaimProvenance::legacy(
            ns("tmdb.movie"),
            Some(MetadataLocale::try_new("en-US").unwrap()),
        );
        let independent_locale = FieldClaimProvenance::legacy(
            ns("tmdb.movie"),
            Some(MetadataLocale::try_new("fr-FR").unwrap()),
        );
        let independent_namespace = FieldClaimProvenance::legacy(ns("other"), None);
        assert_eq!(
            reusable_metadata_evidence(
                &[read_evidence(&complete, 100, None, None)],
                received(150).value()
            ),
            vec![true]
        );
        assert_eq!(
            reusable_metadata_evidence(
                &[read_evidence(&wildcard, 100, None, None)],
                received(150).value()
            ),
            vec![true]
        );
        let evidence = [
            read_evidence(
                &complete,
                100,
                Some(220),
                Some(ProviderResponseReuse::ValidateEveryReuse),
            ),
            read_evidence(&complete, 200, None, None),
            read_evidence(&wildcard, 200, None, None),
            read_evidence(&matching_locale, 200, None, None),
            read_evidence(&independent_locale, 200, None, None),
            read_evidence(&independent_namespace, 200, None, None),
        ];
        assert_eq!(
            reusable_metadata_evidence(&evidence, received(250).value()),
            vec![false, false, false, false, true, true]
        );
        // Only a later known permissive observation, not an unknown receipt,
        // can replace the earlier known restriction for this complete variant.
        let evidence = [
            read_evidence(
                &complete,
                100,
                Some(220),
                Some(ProviderResponseReuse::ValidateEveryReuse),
            ),
            read_evidence(
                &complete,
                200,
                Some(320),
                Some(ProviderResponseReuse::Reusable),
            ),
        ];
        assert_eq!(
            reusable_metadata_evidence(&evidence, received(250).value()),
            vec![false, true]
        );
    }

    #[test]
    fn all_unknown_complete_claims_preserve_fresh_over_newer_stale_resolution() {
        let node = TestNode::new();
        let (record, _, _) = refresh_fixture(&node);
        let observed = chrono::DateTime::from_timestamp_micros(
            (now() - chrono::Duration::seconds(60)).timestamp_micros(),
        ).unwrap();
        let key = field_key(TITLE_FIELD_KEY);
        let provenance = response_provenance("tmdb.movie", "438631");
        let old = FieldClaim::try_new_provider(
            MetadataClaimId::new_v7(),
            record,
            key.clone(),
            "Older fresh historical title",
            provenance.clone(),
            ReceivedAt::from_application_clock(observed),
            Some(observed + chrono::Duration::seconds(120)),
            FieldClaimStatus::Fresh,
        )
        .unwrap();
        let new = FieldClaim::try_new_provider(
            MetadataClaimId::new_v7(),
            record,
            key.clone(),
            "Newer stale historical title",
            provenance,
            ReceivedAt::from_application_clock(observed + chrono::Duration::seconds(10)),
            None,
            FieldClaimStatus::Stale,
        )
        .unwrap();
        let capability = CapabilityKey::ReadMetadataProjection;
        let id = RequestCorrelationId::new_v7();
        {
            let connection = node.kernel.inner.connection.lock().unwrap();
            for claim in [&old, &new] {
                write_field_claim(
                    &connection,
                    node.access.workspace_id(),
                    record,
                    &key,
                    claim,
                    capability,
                    id,
                    None,
                )
                .unwrap();
            }
            let read_at = now();
            let claims = load_field_claims(
                &connection,
                node.access.workspace_id(),
                record,
                &key,
                capability,
                id,
                read_at,
            )
            .unwrap();
            assert_eq!(
                claims.len(),
                2,
                "NULL policy must not preselect only the newest observation"
            );
            let policy = MetadataProjectionPolicy::default_for_profile(node.access.profile_id());
            let expected =
                resolve_profile_field(None, &[old.clone(), new.clone()], &[], &policy, read_at)
                    .unwrap();
            let actual = resolve_profile_field(None, &claims, &[], &policy, read_at).unwrap();
            assert_eq!(actual, expected);
            assert_eq!(actual.value(), Some("Older fresh historical title"));
            assert_eq!(actual.provenance().unwrap().claim_id(), old.claim_id());
        }
        let before = durable_rows(&node);
        let view = node
            .kernel
            .authorize_and_read_projection(ReadMetadataProjectionQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record,
                true,
            ))
            .unwrap();
        assert_eq!(
            view.fields()
                .iter()
                .find(|field| field.field_key() == &key)
                .unwrap()
                .resolved_field()
                .value(),
            Some("Older fresh historical title")
        );
        let listed = node
            .kernel
            .list_records(ListRecordsQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
            ))
            .unwrap()
            .into_records();
        assert_eq!(
            listed
                .iter()
                .find(|value| value.record_id() == record)
                .unwrap()
                .title()
                .value(),
            Some("Older fresh historical title")
        );
        assert_eq!(durable_rows(&node), before);
    }

    #[test]
    fn known_restriction_beyond_256_newer_null_claims_blocks_field_batch_search_and_ratings() {
        for depth in [256, 4096] {
            full_history_restriction_case(depth, "same");
        }
    }

    #[test]
    fn full_history_restrictions_preserve_independent_variants_and_deny_legacy_wildcards() {
        for depth in [256, 4096] {
            for variant in ["source", "locale", "legacy"] {
                full_history_restriction_case(depth, variant);
            }
        }
    }

    fn full_history_restriction_case(depth: i64, variant: &str) {
        let node = TestNode::new();
        let (record, _, _) = refresh_fixture(&node);
        let observed = now() - chrono::Duration::seconds(60);
        let key = field_key(TITLE_FIELD_KEY);
        let policy = ProviderResponseCachePolicy::new(
            ProviderResponseReuse::ValidateEveryReuse,
            observed,
            Duration::ZERO,
            Some(Duration::from_secs(120)),
            None,
        )
        .to_canonical_json();
        let capability = CapabilityKey::ReadMetadataProjection;
        let id = RequestCorrelationId::new_v7();
        {
            let mut connection = node.kernel.inner.connection.lock().unwrap();
            let transaction = connection.transaction().unwrap();
            for index in 0..=depth {
                let fetched = observed + chrono::Duration::microseconds(index);
                let (expires, status) = if index == 0 {
                    (None, FieldClaimStatus::Stale)
                } else {
                    (
                        Some(fetched + chrono::Duration::seconds(3600)),
                        FieldClaimStatus::Fresh,
                    )
                };
                let provenance = if index > 0 && variant == "source" {
                    response_provenance("tmdb.movie", "independent-source")
                } else if index > 0 && variant == "locale" {
                    FieldClaimProvenance::try_new(
                        MetadataProviderId::try_new("tmdb").unwrap(),
                        ns("tmdb.movie"),
                        "438631",
                        Some(MetadataLocale::try_new("fr-FR").unwrap()),
                        None,
                        Some("v3".into()),
                        digest("a"),
                    )
                    .unwrap()
                } else {
                    response_provenance("tmdb.movie", "438631")
                };
                let claim = if index > 0 && variant == "legacy" {
                    FieldClaim::try_new(
                        ns("tmdb.movie"),
                        "Restricted history lighthouse",
                        None,
                        ReceivedAt::from_application_clock(fetched),
                        expires,
                    )
                    .unwrap()
                } else {
                    FieldClaim::try_new_provider(
                        MetadataClaimId::new_v7(),
                        record,
                        key.clone(),
                        "Restricted history lighthouse",
                        provenance.clone(),
                        ReceivedAt::from_application_clock(fetched),
                        expires,
                        status,
                    )
                    .unwrap()
                };
                let rating = RatingClaim::try_new(
                    MetadataClaimId::new_v7(),
                    record,
                    8_750,
                    RatingScale::try_new(0, 10_000).unwrap(),
                    provenance,
                    ReceivedAt::from_application_clock(fetched),
                    expires,
                    status,
                )
                .unwrap();
                let json = (index == 0).then_some(policy.as_str());
                write_field_claim(
                    &transaction,
                    node.access.workspace_id(),
                    record,
                    &key,
                    &claim,
                    capability,
                    id,
                    json,
                )
                .unwrap();
                write_rating_claim(
                    &transaction,
                    node.access.workspace_id(),
                    &rating,
                    capability,
                    id,
                    json,
                )
                .unwrap();
            }
            transaction.commit().unwrap();
            for (table, ordering, filter) in [
                (
                    "metadata_claim_provenance",
                    "evidence.fetched_at DESC, evidence.source DESC",
                    "AND evidence.field_key='core.title'",
                ),
                (
                    "metadata_rating_claims",
                    "evidence.fetched_at DESC, evidence.claim_id DESC",
                    "",
                ),
            ] {
                let count: i64 = connection.query_row(&format!("SELECT COUNT(*) FROM {table} evidence WHERE evidence.record_id=?1 {filter}"), [record.to_string()], |row| row.get(0)).unwrap();
                assert_eq!(count, depth + 1);
                let mut statement = connection.prepare(&format!(
                    "SELECT registered.response_policy_json FROM {table} evidence JOIN metadata_claims registered ON registered.claim_id=evidence.claim_id WHERE evidence.record_id=?1 {filter} ORDER BY {ordering} LIMIT 256"
                )).unwrap();
                let top = statement
                    .query_map([record.to_string()], |row| row.get::<_, Option<String>>(0))
                    .unwrap()
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .unwrap();
                assert_eq!(top.len(), 256);
                assert!(
                    top.iter().all(Option::is_none),
                    "{table}: restriction must lie beyond the payload window"
                );
            }
        }
        let independent = matches!(variant, "source" | "locale");
        let expected_title = independent.then_some("Restricted history lighthouse");
        let before = durable_rows(&node);
        {
            let connection = node.kernel.inner.connection.lock().unwrap();
            let read_at = now();
            assert_eq!(
                load_field_claims(
                    &connection,
                    node.access.workspace_id(),
                    record,
                    &key,
                    capability,
                    id,
                    read_at
                )
                .unwrap()
                .is_empty(),
                !independent,
                "depth={depth}, variant={variant}"
            );
            assert_eq!(
                load_rating_claims(
                    &connection,
                    node.access.workspace_id(),
                    record,
                    capability,
                    id,
                    read_at
                )
                .unwrap()
                .is_empty(),
                !independent,
                "rating depth={depth}, variant={variant}"
            );
            let keys = [
                key.clone(),
                field_key(ORIGINAL_TITLE_FIELD_KEY),
                field_key(OVERVIEW_FIELD_KEY),
                field_key(POSTER_FIELD_KEY),
                field_key(RELEASE_YEAR_FIELD_KEY),
            ];
            let batch = load_record_metadata_batch(
                &connection,
                node.access.workspace_id(),
                node.access.profile_id(),
                &[record],
                &keys,
                capability,
                id,
            )
            .unwrap();
            assert_eq!(
                batch.resolve(record, &key, capability, id).unwrap().value(),
                expected_title
            );
            let postings: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM local_search_grams WHERE record_id=?1 AND gram='lig'",
                    [record.to_string()],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(
                postings > 0,
                "local Search must reject real retained postings"
            );
        }
        let search = node
            .kernel
            .search_local_records(&LocalSearchRequest {
                correlation_id: RequestCorrelationId::new_v7(),
                access: node.access.into(),
                query: SearchQuery::try_new("lighthouse").unwrap(),
                grains: Vec::new(),
                after: None,
            })
            .unwrap();
        assert_eq!(
            search.records.is_empty(), !independent,
            "old known restriction must govern final Search disclosure: depth={depth}, variant={variant}"
        );
        let view = node
            .kernel
            .authorize_and_read_projection(ReadMetadataProjectionQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record,
                true,
            ))
            .unwrap();
        assert_eq!(
            view.fields()
                .iter()
                .find(|field| field.field_key() == &key)
                .unwrap()
                .resolved_field()
                .value(),
            expected_title
        );
        assert_eq!(view.ratings().is_empty(), !independent);
        assert_eq!(
            durable_rows(&node),
            before,
            "all observations remain immutable"
        );
        let counts_before = {
            let connection = node.kernel.inner.connection.lock().unwrap();
            ["metadata_claim_provenance", "metadata_rating_claims"].map(|table| {
                connection.query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE record_id=?1"),
                    [record.to_string()],
                    |row| row.get::<_, i64>(0),
                ).unwrap()
            })
        };
        let (root, access) = node.into_stopped();
        let reopened = crate::SqliteKernel::open(root.path()).unwrap();
        let view = reopened
            .authorize_and_read_projection(ReadMetadataProjectionQuery::new(
                RequestCorrelationId::new_v7(),
                access,
                record,
                true,
            ))
            .unwrap();
        assert_eq!(
            view.fields()
                .iter()
                .find(|field| field.field_key() == &key)
                .unwrap()
                .resolved_field()
                .value(),
            expected_title
        );
        assert_eq!(view.ratings().is_empty(), !independent);
        let search = reopened
            .search_local_records(&LocalSearchRequest {
                correlation_id: RequestCorrelationId::new_v7(),
                access: access.into(),
                query: SearchQuery::try_new("lighthouse").unwrap(),
                grains: Vec::new(),
                after: None,
            })
            .unwrap();
        assert_eq!(search.records.is_empty(), !independent);
        let connection = reopened.inner.connection.lock().unwrap();
        for (table, expected) in ["metadata_claim_provenance", "metadata_rating_claims"].into_iter().zip(counts_before) {
            let count: i64 = connection
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE record_id=?1"),
                    [record.to_string()],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, expected, "reopen preserves {table}");
        }
    }

    #[test]
    fn buried_restriction_denies_saved_null_receipt_and_fresh_cache_after_reopen() {
        let node = TestNode::new();
        let (record, provider, prepared) = refresh_fixture(&node);
        let key = field_key(TITLE_FIELD_KEY);
        let observed = received(now().timestamp() - 60).value();
        let capability = CapabilityKey::RefreshMetadataClaims;
        let id = RequestCorrelationId::new_v7();
        let claims_at = |ordinal| {
            let fetched = observed + chrono::Duration::microseconds(ordinal);
            let expires = (ordinal != 0).then_some(fetched + chrono::Duration::hours(1));
            let status = if ordinal == 0 { FieldClaimStatus::Stale } else { FieldClaimStatus::Fresh };
            let provenance = response_provenance("tmdb.movie", "438631");
            (
                FieldClaim::try_new_provider(
                    MetadataClaimId::new_v7(), record, key.clone(), "Retained receipt lighthouse",
                    provenance.clone(), ReceivedAt::from_application_clock(fetched), expires, status,
                ).unwrap(),
                RatingClaim::try_new(
                    MetadataClaimId::new_v7(), record, 8_750,
                    RatingScale::try_new(0, 10_000).unwrap(), provenance,
                    ReceivedAt::from_application_clock(fetched), expires, status,
                ).unwrap(),
            )
        };
        let (field, rating) = {
            let mut connection = node.kernel.inner.connection.lock().unwrap();
            let tx = connection.transaction().unwrap();
            let mut latest = None;
            for ordinal in 1..=256 {
                let (field, rating) = claims_at(ordinal);
                write_field_claim(&tx, node.access.workspace_id(), record, &key,
                    &field, capability, id, None).unwrap();
                write_rating_claim(&tx, node.access.workspace_id(), &rating,
                    capability, id, None).unwrap();
                latest = Some((field, rating));
            }
            tx.commit().unwrap();
            latest.unwrap()
        };
        let cache_key = MetadataCacheKey::try_new(
            provider.clone(), None, record, "metadata/movie", prepared.grain(),
            ns("tmdb.movie"), "438631", Some(MetadataLocale::try_new("en-US").unwrap()),
            None, MetadataFieldGroup::BasicInfo, prepared.settings_fingerprint().clone(),
            digest("a"), 1, MetadataCachePurpose::MetadataEnrichment,
            "tmdb_attribution_required", MetadataDataClassification::Public,
        ).unwrap();
        let cache = MetadataCacheEntry::try_new(
            cache_key.clone(), vec![field.claim_id()],
            ReceivedAt::from_application_clock(field.fetched_at()),
            field.expires_at().unwrap(), field.expires_at().unwrap(), field.expires_at().unwrap(),
        ).unwrap();
        {
            let connection = node.kernel.inner.connection.lock().unwrap();
            write_metadata_cache_entry(&connection, node.access.workspace_id(), &cache,
                capability, id).unwrap();
        }
        let cached = |access| ReadCachedMetadataRefreshCommand::new(
            id, access, prepared.clone(), vec![cache_key.clone()],
        );
        assert!(node.kernel.authorize_and_read_cached_refresh(cached(node.access)).unwrap().is_some(),
            "the original cache entry is real and fresh before the restriction");
        let outcome = RefreshMetadataClaimsOutcome::new(
            vec![FieldClaimView::new(field.clone(), FieldClaimStatus::Fresh)],
            vec![RatingClaimView::new(rating.clone(), FieldClaimStatus::Fresh)],
            Vec::new(), Vec::new(), Vec::new(),
        );
        let operation = fasti_domain::OperationId::new_v7();
        let command = |access| CommitMetadataRefreshReceiptCommand::new(
            id, access, operation, digest("c"), record, provider.clone(), outcome.clone(),
        );
        let read = |access| ReadMetadataRefreshReceiptCommand::new(
            id, access, operation, digest("c"), record, provider.clone(),
        );
        let saved = node.kernel.authorize_and_commit_refresh_receipt(command(node.access)).unwrap();
        assert_eq!(node.kernel.authorize_and_read_refresh_receipt(read(node.access)).unwrap(), Some(saved));
        {
            // A valid imported history may add an older known observation after
            // NULL-policy history. Do not rewrite the saved receipt or its claims.
            let (restricted_field, restricted_rating) = claims_at(0);
            let policy = ProviderResponseCachePolicy::new(
                ProviderResponseReuse::ValidateEveryReuse, observed, Duration::ZERO,
                Some(Duration::from_secs(120)), None,
            ).to_canonical_json();
            let mut connection = node.kernel.inner.connection.lock().unwrap();
            let tx = connection.transaction().unwrap();
            write_field_claim(&tx, node.access.workspace_id(), record, &key,
                &restricted_field, capability, id, Some(&policy)).unwrap();
            write_rating_claim(&tx, node.access.workspace_id(), &restricted_rating,
                capability, id, Some(&policy)).unwrap();
            tx.commit().unwrap();
            for (table, order) in [
                ("metadata_claim_provenance", "fetched_at DESC, source DESC"),
                ("metadata_rating_claims", "fetched_at DESC, claim_id DESC"),
            ] {
                let filter = if table == "metadata_claim_provenance" { "AND field_key='core.title'" } else { "" };
                let count: i64 = connection.query_row(&format!(
                    "SELECT COUNT(*) FROM (SELECT claim_id FROM {table} WHERE record_id=?1 {filter} ORDER BY {order} LIMIT 256) selected JOIN metadata_claims registry ON registry.claim_id=selected.claim_id WHERE registry.response_policy_json IS NULL"
                ), [record.to_string()], |row| row.get(0)).unwrap();
                assert_eq!(count, 256, "{table}: the restriction is outside the selected payload");
            }
        }
        let check = |kernel: &SqliteKernel, access: fasti_application::RequestAccessContext| {
            let receipt_bytes = || kernel.inner.connection.lock().unwrap().query_row(
                "SELECT response_json FROM metadata_refresh_receipts WHERE operation_id=?1",
                [operation.to_string()], |row| row.get::<_, String>(0),
            ).unwrap();
            let original = receipt_bytes();
            let mutations = Arc::new(Mutex::new(Vec::new()));
            {
                let connection = kernel.inner.connection.lock().unwrap();
                connection.flush_prepared_statement_cache();
                let observed = Arc::clone(&mutations);
                connection.authorizer(Some(move |context: AuthContext<'_>| {
                    match context.action {
                        AuthAction::Insert { table_name } | AuthAction::Delete { table_name }
                        | AuthAction::Update { table_name, .. } => observed.lock().unwrap().push(table_name.to_owned()),
                        _ => {}
                    }
                    Authorization::Allow
                })).unwrap();
            }
            let replay = kernel.authorize_and_read_refresh_receipt(read(access));
            let recommit = kernel.authorize_and_commit_refresh_receipt(command(access));
            let cache_result = kernel.authorize_and_read_cached_refresh(cached(access));
            kernel.inner.connection.lock().unwrap()
                .authorizer(None::<fn(AuthContext<'_>) -> Authorization>).unwrap();
            assert_eq!(replay.unwrap_err().code(), ProblemCode::MetadataClaimStale);
            assert_eq!(recommit.unwrap_err().code(), ProblemCode::MetadataClaimStale);
            assert!(cache_result.unwrap().is_none(), "fresh cache cannot disclose blocked claim references");
            assert!(mutations.lock().unwrap().is_empty(), "denial must execute zero DML");
            assert_eq!(receipt_bytes(), original, "denial preserves exact receipt bytes");
        };
        let before = durable_rows(&node);
        check(&node.kernel, node.access);
        assert_eq!(durable_rows(&node), before, "denial retains all payload and receipt rows");
        let (root, access) = node.into_stopped();
        let reopened = SqliteKernel::open(root.path()).unwrap();
        check(&reopened, access);
    }

    #[test]
    fn historical_coordinate_casing_keeps_newer_permission_over_older_restriction() {
        for spelling in ["locale", "region", "both"] {
            let node = TestNode::new();
            let (record, _, _) = refresh_fixture(&node);
            let key = field_key(TITLE_FIELD_KEY);
            let observed = received(now().timestamp() - 60).value();
            let capability = CapabilityKey::ReadMetadataProjection;
            let id = RequestCorrelationId::new_v7();
            let mut ids = Vec::new();
            {
                let mut connection = node.kernel.inner.connection.lock().unwrap();
                let tx = connection.transaction().unwrap();
                for index in 0..3 {
                    let fetched = observed + chrono::Duration::seconds(index * 10);
                    let restricted = index == 0;
                    let expiry = (!restricted)
                        .then_some(fetched + chrono::Duration::seconds(120));
                    let status = if restricted {
                        FieldClaimStatus::Stale
                    } else {
                        FieldClaimStatus::Fresh
                    };
                    let provenance = FieldClaimProvenance::try_new(
                        MetadataProviderId::try_new("tmdb").unwrap(),
                        ns("tmdb.movie"),
                        "438631",
                        Some(MetadataLocale::try_new("en-us").unwrap()),
                        Some(fasti_domain::MetadataRegion::try_new("IE").unwrap()),
                        Some("v3".into()),
                        digest("a"),
                    )
                    .unwrap();
                    let field = FieldClaim::try_new_provider(
                        MetadataClaimId::new_v7(), record, key.clone(),
                        if index == 2 { "Historical casing lighthouse" } else { "Observed title" },
                        provenance.clone(), ReceivedAt::from_application_clock(fetched),
                        expiry, status,
                    )
                    .unwrap();
                    let rating = RatingClaim::try_new(
                        MetadataClaimId::new_v7(), record, 8_750,
                        RatingScale::try_new(0, 10_000).unwrap(), provenance,
                        ReceivedAt::from_application_clock(fetched), expiry, status,
                    )
                    .unwrap();
                    let policy = (index != 2).then(|| ProviderResponseCachePolicy::new(
                        if restricted { ProviderResponseReuse::ValidateEveryReuse }
                        else { ProviderResponseReuse::Reusable },
                        fetched, Duration::ZERO, Some(Duration::from_secs(120)), None,
                    ).to_canonical_json());
                    write_field_claim(&tx, node.access.workspace_id(), record, &key,
                        &field, capability, id, policy.as_deref()).unwrap();
                    write_rating_claim(&tx, node.access.workspace_id(), &rating,
                        capability, id, policy.as_deref()).unwrap();
                    ids.push((field.claim_id(), rating.claim_id()));
                }
                // Restore validates these spellings through domain constructors,
                // then retains the original row strings. Model that historical
                // shape only: capture and restore the actual immutable triggers.
                let locale = if spelling == "region" { "en-us" } else { "en-US" };
                let region = if spelling == "locale" { "IE" } else { "ie" };
                assert_eq!(MetadataLocale::try_new(locale).unwrap().as_str(), "en-us");
                assert_eq!(fasti_domain::MetadataRegion::try_new(region).unwrap().as_str(), "IE");
                let mut triggers = Vec::new();
                for name in ["metadata_field_claims_immutable_update",
                    "metadata_claim_provenance_immutable_update",
                    "metadata_rating_claims_immutable_update"] {
                    let sql: String = tx.query_row(
                        "SELECT sql FROM sqlite_schema WHERE type='trigger' AND name=?1",
                        [name], |row| row.get(0),
                    ).unwrap();
                    tx.execute_batch(&format!("DROP TRIGGER {name}")).unwrap();
                    triggers.push(sql);
                }
                assert_eq!(tx.execute(
                    "UPDATE metadata_field_claims SET locale=?1 WHERE record_id=?2 AND field_key=?3 AND source='tmdb.movie' AND fetched_at=?4",
                    params![locale, record.to_string(), key.as_str(), timestamp(observed)],
                ).unwrap(), 1);
                assert_eq!(tx.execute(
                    "UPDATE metadata_claim_provenance SET region=?1 WHERE claim_id=?2",
                    params![region, ids[0].0.to_string()],
                ).unwrap(), 1);
                assert_eq!(tx.execute(
                    "UPDATE metadata_rating_claims SET locale=?1, region=?2 WHERE claim_id=?3",
                    params![locale, region, ids[0].1.to_string()],
                ).unwrap(), 1);
                for sql in triggers { tx.execute_batch(&sql).unwrap(); }
                tx.commit().unwrap();
            }
            let before = durable_rows(&node);
            {
                let connection = node.kernel.inner.connection.lock().unwrap();
                let read_at = now();
                let fields = load_field_claims(&connection, node.access.workspace_id(),
                    record, &key, capability, id, read_at).unwrap();
                let ratings = load_rating_claims(&connection, node.access.workspace_id(),
                    record, capability, id, read_at).unwrap();
                assert!(fields.iter().any(|claim| claim.claim_id() == ids[2].0),
                    "{spelling}: newer permission must keep unknown field evidence readable");
                assert!(ratings.iter().any(|claim| claim.claim_id() == ids[2].1),
                    "{spelling}: newer permission must keep unknown rating evidence readable");
                assert!(!fields.iter().any(|claim| claim.claim_id() == ids[0].0));
                assert!(!ratings.iter().any(|claim| claim.claim_id() == ids[0].1));
                let keys = [key.clone(), field_key(ORIGINAL_TITLE_FIELD_KEY),
                    field_key(OVERVIEW_FIELD_KEY), field_key(POSTER_FIELD_KEY),
                    field_key(RELEASE_YEAR_FIELD_KEY)];
                let batch = load_record_metadata_batch(&connection, node.access.workspace_id(),
                    node.access.profile_id(), &[record], &keys, capability, id).unwrap();
                assert_eq!(batch.resolve(record, &key, capability, id).unwrap().value(),
                    Some("Historical casing lighthouse"), "{spelling}: batch reader");
            }
            assert_eq!(durable_rows(&node), before, "{spelling}: reads retain original bytes");
        }
    }

    #[test]
    fn refresh_rejects_policy_inconsistent_cache_timestamps_before_any_payload_statement() {
        for invalid in ["created", "fresh", "refreshing", "stale_error"] {
            let node = TestNode::new();
            let (record, provider, prepared) = refresh_fixture(&node);
            let state = provider_state(1);
            node.kernel
                .put_provider_capability_state(node.access.workspace_id(), state.clone())
                .unwrap();
            let field = response_field(TITLE_FIELD_KEY, "tmdb.movie", "438631", 100);
            let key = MetadataCacheKey::try_new(
                provider.clone(),
                Some(state.capability_version()),
                record,
                "metadata/movie",
                prepared.grain(),
                ns(prepared.identifier().namespace()),
                prepared.identifier().value(),
                Some(MetadataLocale::try_new("en-US").unwrap()),
                None,
                MetadataFieldGroup::BasicInfo,
                prepared.settings_fingerprint().clone(),
                digest("a"),
                1,
                MetadataCachePurpose::MetadataEnrichment,
                "tmdb_attribution_required",
                MetadataDataClassification::Public,
            )
            .unwrap();
            let fresh = if invalid == "fresh" { 221 } else { 220 };
            let refreshing = if matches!(invalid, "fresh" | "refreshing") {
                221
            } else {
                220
            };
            let stale = if invalid == "created" { 220 } else { 221 };
            let cache = MetadataCacheEntry::try_new(
                key,
                vec![field.claim().claim_id()],
                received(if invalid == "created" { 101 } else { 100 }),
                received(fresh).value(),
                received(refreshing).value(),
                received(stale).value(),
            )
            .expect("domain-valid timestamps still violate the narrower observed response policy");
            let command = CommitMetadataRefreshCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                fasti_domain::OperationId::new_v7(),
                digest("c"),
                prepared,
                provider.clone(),
                state,
                vec![field],
                Vec::new(),
                vec![cache],
                MetadataAttribution::try_new(
                    provider,
                    "Metadata supplied by TMDB",
                    "https://developer.themoviedb.org/",
                )
                .unwrap(),
                response_policy(ProviderResponseReuse::ValidateWhenStale),
            );
            let before = durable_rows(&node);
            let (result, mutations) =
                observe_mutations(&node, || node.kernel.authorize_and_commit_refresh(command));
            assert!(result.is_err(), "cache case {invalid}");
            assert!(
                mutations.is_empty(),
                "cache case {invalid}: admitted {mutations:?}"
            );
            assert_eq!(durable_rows(&node), before);
        }
    }
}
