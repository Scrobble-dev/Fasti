mod negative_tests {
    use super::*;
    use fasti_application::{
        provider_candidate_metadata_fields, provider_metadata_response_locale,
        ProviderMetadataField, ProviderResponseCachePolicy, ProviderResponseReuse,
    };
    use fasti_domain::{
        FieldClaim, FieldClaimProvenance, FieldClaimStatus, FieldKey, MetadataClaimId,
        MetadataLocale, MetadataProviderId, MetadataRegion, NamespaceKey, ReceivedAt,
    };

    fn refetch_fixture() -> (
        TestNode,
        SearchCandidateActionCommand,
        SearchCandidateActionPreparation,
        Vec<ProviderMetadataField>,
        ProviderResponseCachePolicy,
    ) {
        let (node, _, mut command, _) = fixture();
        node.kernel
            .put_provider_capability_state(
                node.access.workspace_id(),
                state_for("metadata.read", 1),
            )
            .unwrap();
        command.evidence_mode = SearchCandidateEvidenceMode::Refetch;
        let prepared = node
            .kernel
            .prepare_search_candidate_action(&command)
            .unwrap();
        let SearchCandidateActionPreparation::Refetch(details) = &prepared else {
            panic!("refetch must capture current metadata.read authority")
        };
        let fetched = chrono::DateTime::from_timestamp_micros(now().timestamp_micros()).unwrap();
        let fields = provider_candidate_metadata_fields(
            details.candidate.receipt.candidate(),
            provider_metadata_response_locale("tmdb", details.candidate.context.locale()),
            None,
            &Sha256Digest::from_bytes(&[9; 32]),
            ReceivedAt::from_application_clock(fetched),
            Some(fetched + chrono::Duration::seconds(fasti_domain::METADATA_FRESH_SECONDS)),
            FieldClaimStatus::Fresh,
        )
        .unwrap();
        let policy = ProviderResponseCachePolicy::new(
            ProviderResponseReuse::Reusable,
            fetched,
            std::time::Duration::ZERO,
            None,
            None,
        );
        (node, command, prepared, fields, policy)
    }

    fn replace_claim(
        field: &ProviderMetadataField,
        value: &str,
        provenance: FieldClaimProvenance,
        fetched: chrono::DateTime<chrono::Utc>,
        expiry: Option<chrono::DateTime<chrono::Utc>>,
        status: FieldClaimStatus,
    ) -> ProviderMetadataField {
        ProviderMetadataField::new(
            field.field_key().clone(),
            FieldClaim::try_new_unbound_provider(
                MetadataClaimId::new_v7(),
                value,
                provenance,
                ReceivedAt::from_application_clock(fetched),
                expiry,
                status,
            )
            .unwrap(),
        )
    }

    fn provenance(
        provider: &str,
        namespace: &str,
        identifier: &str,
        locale: Option<&str>,
        region: Option<&str>,
        digest: u8,
    ) -> FieldClaimProvenance {
        FieldClaimProvenance::try_new(
            MetadataProviderId::try_new(provider).unwrap(),
            NamespaceKey::try_new(namespace).unwrap(),
            identifier,
            locale.map(|value| MetadataLocale::try_new(value).unwrap()),
            region.map(|value| MetadataRegion::try_new(value).unwrap()),
            None,
            Sha256Digest::from_bytes(&[digest; 32]),
        )
        .unwrap()
    }

    #[test]
    fn refetch_action_uses_effective_locale_and_new_response_without_rewriting_search_evidence() {
        let (node, command, prepared, fields, policy) = refetch_fixture();
        let SearchCandidateActionPreparation::Refetch(details) = &prepared else {
            unreachable!()
        };
        assert_eq!(details.candidate.context.locale(), None);
        let result = node
            .kernel
            .commit_search_candidate_action(&command, &prepared, Some((&fields, &policy)))
            .unwrap();
        assert_eq!(result.evidence_mode, SearchCandidateEvidenceMode::Refetch);
        assert_eq!(result.provenance.locale().unwrap().as_str(), "en-us");
        assert_eq!(
            result.provenance.evidence_digest(),
            Some(&Sha256Digest::from_bytes(&[9; 32]))
        );
        assert_eq!(
            result.search_response_digest,
            *details.candidate.receipt.response_digest()
        );
        assert_ne!(
            result.provenance.evidence_digest(),
            Some(&result.search_response_digest)
        );
        assert_eq!(result.fetched_at, fields[0].claim().fetched_at());
        assert_eq!(result.expires_at, fields[0].claim().expires_at());
        assert_eq!(act(&node, &command).unwrap(), result);
    }

    #[test]
    fn refetch_action_rejects_authority_away_and_back_then_accepts_current_preparation() {
        let (node, command, prepared, fields, policy) = refetch_fixture();
        let changed = ProviderCapabilityState::try_new(
            ProviderId::try_new("tmdb").unwrap(),
            ProviderCapabilityId::try_new("metadata.read").unwrap(),
            ProviderCapabilityStatus::Available,
            2,
            CredentialRequirement::BearerToken,
            Some(CredentialReference::try_new("secret:tmdb-test").unwrap()),
            ProviderCredentialStatus::StoredUnverified,
            ConfigurationDigest::parse("b".repeat(64)).unwrap(),
            ProviderCheckMetadata::never_run(),
            ProviderCheckMetadata::never_run(),
        )
        .unwrap();
        node.kernel
            .put_provider_capability_state(node.access.workspace_id(), changed)
            .unwrap();
        node.kernel
            .put_provider_capability_state(
                node.access.workspace_id(),
                state_for("metadata.read", 3),
            )
            .unwrap();
        let before = rows(&node, MUTATION_TABLES);
        let error = node
            .kernel
            .commit_search_candidate_action(&command, &prepared, Some((&fields, &policy)))
            .unwrap_err();
        assert_eq!(error.code(), ProblemCode::Forbidden);
        assert_eq!(rows(&node, MUTATION_TABLES), before);
        let current = node
            .kernel
            .prepare_search_candidate_action(&command)
            .unwrap();
        node.kernel
            .commit_search_candidate_action(&command, &current, Some((&fields, &policy)))
            .unwrap();
    }

    #[test]
    fn zero_freshness_refetch_saves_original_policy_and_replays_audit_without_new_search_authority()
    {
        for reuse in [
            ProviderResponseReuse::ValidateEveryReuse,
            ProviderResponseReuse::Reusable,
        ] {
            let (node, command, prepared, fields, original_policy) = refetch_fixture();
            let policy = ProviderResponseCachePolicy::new(
                reuse,
                original_policy.received_at(),
                std::time::Duration::from_secs(90),
                Some(std::time::Duration::from_secs(30)),
                Some(std::time::Duration::from_secs(60)),
            );
            let fields: Vec<_> = fields
                .iter()
                .map(|field| {
                    replace_claim(
                        field,
                        field.claim().value(),
                        field.claim().provenance().clone(),
                        original_policy.received_at(),
                        None,
                        FieldClaimStatus::Stale,
                    )
                })
                .collect();
            let receipt = node
                .kernel
                .commit_search_candidate_action(&command, &prepared, Some((&fields, &policy)))
                .expect("zero-freshness refetch persists accurate historical claim status");
            assert_eq!(receipt.initial_status, FieldClaimStatus::Stale);
            assert_eq!(receipt.expires_at, None);
            assert_eq!(receipt.fetched_at, policy.received_at());
            {
                let connection = node.kernel.inner.connection.lock().unwrap();
                for field in &fields {
                    let stored: String = connection
                        .query_row(
                            "SELECT response_policy_json FROM metadata_claims WHERE claim_id = ?1",
                            [field.claim().claim_id().to_string()],
                            |row| row.get(0),
                        )
                        .unwrap();
                    assert_eq!(stored, policy.to_canonical_json());
                }
            }
            remove_scope(&node, "metadata_search");
            let before = rows(&node, MUTATION_TABLES);
            let replay = node
                .kernel
                .prepare_search_candidate_action(&command)
                .unwrap();
            assert!(
                matches!(&replay, SearchCandidateActionPreparation::Replay(saved) if **saved == receipt)
            );
            assert_eq!(
                node.kernel
                    .commit_search_candidate_action(&command, &replay, None)
                    .unwrap(),
                receipt
            );
            assert_eq!(rows(&node, MUTATION_TABLES), before);
        }
    }

    #[test]
    fn refetch_action_rejects_hostile_fields_without_any_durable_partial_mutation() {
        for case in [
            "provider",
            "namespace",
            "identifier",
            "locale",
            "absent_locale",
            "region",
            "mixed_digest",
            "mixed_time",
            "mixed_expiry",
            "mixed_status",
            "old_time",
            "future_time",
            "no_expiry",
            "long_expiry",
            "stale",
            "empty",
            "missing_title",
            "oversize_batch",
            "duplicate",
            "long_title",
            "long_original",
            "unsafe_poster",
            "invalid_year",
            "unknown_field",
        ] {
            let (node, command, prepared, mut fields, policy) = refetch_fixture();
            let first = fields[0].claim();
            let mut source = first.provenance().clone();
            let mut fetched = first.fetched_at();
            let mut expiry = first.expires_at();
            let mut status = first.initial_status();
            match case {
                "provider" => {
                    source = provenance("google_books", "tmdb.movie", "42", Some("en-us"), None, 9)
                }
                "namespace" => source = provenance("tmdb", "tmdb.tv", "42", Some("en-us"), None, 9),
                "identifier" => {
                    source = provenance("tmdb", "tmdb.movie", "43", Some("en-us"), None, 9)
                }
                "locale" => source = provenance("tmdb", "tmdb.movie", "42", Some("fr-fr"), None, 9),
                "absent_locale" => source = provenance("tmdb", "tmdb.movie", "42", None, None, 9),
                "region" => {
                    source = provenance("tmdb", "tmdb.movie", "42", Some("en-us"), Some("FR"), 9)
                }
                "old_time" => {
                    fetched -= chrono::Duration::days(1);
                    expiry = Some(fetched + chrono::Duration::seconds(120));
                }
                "future_time" => {
                    fetched += chrono::Duration::days(1);
                    expiry = Some(fetched + chrono::Duration::seconds(120));
                }
                "no_expiry" => expiry = None,
                "long_expiry" => expiry = Some(fetched + chrono::Duration::days(2)),
                "stale" => status = FieldClaimStatus::Stale,
                _ => {}
            }
            fields = fields
                .iter()
                .map(|field| {
                    replace_claim(
                        field,
                        field.claim().value(),
                        source.clone(),
                        fetched,
                        expiry,
                        status,
                    )
                })
                .collect();
            match case {
                "mixed_digest" => {
                    fields[1] = replace_claim(
                        &fields[1],
                        fields[1].claim().value(),
                        provenance("tmdb", "tmdb.movie", "42", Some("en-us"), None, 8),
                        fetched,
                        expiry,
                        status,
                    )
                }
                "mixed_time" => {
                    fields[1] = replace_claim(
                        &fields[1],
                        fields[1].claim().value(),
                        source.clone(),
                        fetched + chrono::Duration::seconds(1),
                        expiry,
                        status,
                    )
                }
                "mixed_expiry" => {
                    fields[1] = replace_claim(
                        &fields[1],
                        fields[1].claim().value(),
                        source.clone(),
                        fetched,
                        expiry.map(|at| at - chrono::Duration::seconds(1)),
                        status,
                    )
                }
                "mixed_status" => {
                    fields[1] = replace_claim(
                        &fields[1],
                        fields[1].claim().value(),
                        source.clone(),
                        fetched,
                        expiry,
                        FieldClaimStatus::Stale,
                    )
                }
                "empty" => fields.clear(),
                "missing_title" => {
                    fields.remove(0);
                }
                "oversize_batch" => fields = vec![fields[0].clone(); 17],
                "duplicate" => fields.push(fields[0].clone()),
                "long_title" => {
                    fields[0] = replace_claim(
                        &fields[0],
                        &"a".repeat(513),
                        source.clone(),
                        fetched,
                        expiry,
                        status,
                    )
                }
                "long_original" => {
                    fields[1] = replace_claim(
                        &fields[1],
                        &"a".repeat(513),
                        source.clone(),
                        fetched,
                        expiry,
                        status,
                    )
                }
                "unsafe_poster" | "invalid_year" | "unknown_field" => {
                    let (key, value) = match case {
                        "unsafe_poster" => (
                            fasti_domain::POSTER_FIELD_KEY,
                            "https://evil.example/poster.jpg",
                        ),
                        "invalid_year" => ("core.release_year", "999"),
                        _ => ("test.unknown", "value"),
                    };
                    let field =
                        replace_claim(&fields[0], value, source.clone(), fetched, expiry, status);
                    fields.push(ProviderMetadataField::new(
                        FieldKey::try_new(key).unwrap(),
                        field.claim().clone(),
                    ));
                }
                _ => {}
            }
            let before = rows(&node, MUTATION_TABLES);
            let error = node.kernel.commit_search_candidate_action(
                &command,
                &prepared,
                Some((&fields, &policy)),
            );
            assert!(error.is_err(), "accepted hostile case {case}");
            assert_eq!(
                rows(&node, MUTATION_TABLES),
                before,
                "partial mutation for {case}"
            );
        }
    }

    #[test]
    fn actions_reject_wrong_preparation_branch_snapshot_and_forged_replay() {
        let (node, command, prepared, fields, policy) = refetch_fixture();
        let SearchCandidateActionPreparation::Refetch(details) = &prepared else {
            unreachable!()
        };
        let before = rows(&node, MUTATION_TABLES);
        let cached = SearchCandidateActionPreparation::Cached(details.candidate.clone());
        assert_eq!(
            node.kernel
                .commit_search_candidate_action(&command, &cached, Some((&fields, &policy)))
                .unwrap_err()
                .code(),
            ProblemCode::Forbidden
        );
        assert!(node
            .kernel
            .commit_search_candidate_action(&command, &prepared, None)
            .is_err());
        let mut altered = details.clone();
        altered.provider_authority_fingerprint = Sha256Digest::from_bytes(&[0; 32]);
        assert_eq!(
            node.kernel
                .commit_search_candidate_action(
                    &command,
                    &SearchCandidateActionPreparation::Refetch(altered),
                    Some((&fields, &policy))
                )
                .unwrap_err()
                .code(),
            ProblemCode::Forbidden
        );
        let mut altered = details.clone();
        let original = &details.candidate.receipt;
        altered.candidate.receipt = SearchCandidateReceipt::new(
            original.id(),
            original.partition().clone(),
            candidate("43"),
            original.response_digest().clone(),
            original.lifetime().clone(),
        );
        assert_eq!(
            node.kernel
                .commit_search_candidate_action(
                    &command,
                    &SearchCandidateActionPreparation::Refetch(altered),
                    Some((&fields, &policy)),
                )
                .unwrap_err()
                .code(),
            ProblemCode::Forbidden
        );
        let mut other_command = command.clone();
        other_command.evidence_mode = SearchCandidateEvidenceMode::Cached;
        assert!(node
            .kernel
            .commit_search_candidate_action(&other_command, &cached, Some((&fields, &policy)))
            .is_err());
        assert_eq!(rows(&node, MUTATION_TABLES), before);

        let receipt = node
            .kernel
            .commit_search_candidate_action(&command, &prepared, Some((&fields, &policy)))
            .unwrap();
        let mut new_command = command.clone();
        new_command.operation_id = OperationId::new_v7();
        let mut forged = receipt;
        forged.operation_id = new_command.operation_id;
        let before = rows(&node, MUTATION_TABLES);
        assert_eq!(
            node.kernel
                .commit_search_candidate_action(
                    &new_command,
                    &SearchCandidateActionPreparation::Replay(Box::new(forged)),
                    None
                )
                .unwrap_err()
                .code(),
            ProblemCode::Forbidden
        );
        assert_eq!(rows(&node, MUTATION_TABLES), before);
    }

    #[test]
    fn search_action_receipt_decoder_rejects_noncanonical_and_inconsistent_evidence() {
        let (node, _, command, _) = fixture();
        let receipt = act(&node, &command).unwrap();
        let canonical = serde_json::to_string(&receipt).unwrap();
        let decode = |json: &str| {
            crate::search_actions::decode_receipt(json, RequestCorrelationId::new_v7())
        };
        assert_eq!(decode(&canonical).unwrap(), receipt);
        let mut google = receipt.clone();
        google.evidence_mode = SearchCandidateEvidenceMode::Refetch;
        google.provider = "google-books".into();
        google.grain = Grain::Edition;
        google.provenance = provenance("google-books", "googlebooks.volume", "42", None, None, 7);
        assert_eq!(
            decode(&serde_json::to_string(&google).unwrap()).unwrap(),
            google
        );
        for hostile in [
            format!("{canonical}{}", " ".repeat(16 * 1024)),
            serde_json::to_string_pretty(&receipt).unwrap(),
            format!("{{\"unknown\":true,{}", &canonical[1..]),
            canonical.replace("\"provenance\":{", "\"provenance\":{\"unknown\":true,"),
            "{}".to_owned(),
        ] {
            assert_eq!(
                decode(&hostile).unwrap_err().code(),
                ProblemCode::IntegrityFailed
            );
        }
        for case in [
            "target",
            "disposition",
            "provider",
            "namespace",
            "identifier",
            "region",
            "digest",
            "expiry",
            "long_expiry",
            "future",
            "invalid_status",
            "stale_refetch",
            "unlocalized_tmdb_refetch",
            "localized_google_books_refetch",
        ] {
            let mut hostile = receipt.clone();
            match case {
                "target" => {
                    hostile.action = SearchRecordAction::Attach(RecordId::new_v7());
                    hostile.disposition = SearchRecordActionDisposition::Attached;
                }
                "disposition" => hostile.disposition = SearchRecordActionDisposition::Attached,
                "provider" => hostile.provider = "google_books".into(),
                "namespace" => {
                    hostile.provenance = provenance("tmdb", "tmdb.tv", "42", None, None, 7)
                }
                "identifier" => {
                    hostile.provenance =
                        provenance("tmdb", "tmdb.movie", "not-an-id", None, None, 7)
                }
                "region" => {
                    hostile.provenance = provenance("tmdb", "tmdb.movie", "42", None, Some("FR"), 7)
                }
                "digest" => {
                    hostile.provenance = provenance("tmdb", "tmdb.movie", "42", None, None, 8)
                }
                "expiry" => hostile.expires_at = Some(hostile.fetched_at),
                "long_expiry" => {
                    hostile.expires_at = Some(hostile.fetched_at + chrono::Duration::seconds(121))
                }
                "future" => {
                    hostile.fetched_at = hostile.committed_at + chrono::Duration::seconds(1)
                }
                "invalid_status" => hostile.initial_status = FieldClaimStatus::Invalid,
                "stale_refetch" => {
                    hostile.evidence_mode = SearchCandidateEvidenceMode::Refetch;
                    hostile.initial_status = FieldClaimStatus::Stale;
                    hostile.expires_at = None;
                }
                "unlocalized_tmdb_refetch" => {
                    hostile.evidence_mode = SearchCandidateEvidenceMode::Refetch;
                }
                "localized_google_books_refetch" => {
                    hostile.evidence_mode = SearchCandidateEvidenceMode::Refetch;
                    hostile.provider = "google-books".into();
                    hostile.grain = Grain::Edition;
                    hostile.provenance = provenance(
                        "google-books",
                        "googlebooks.volume",
                        "42",
                        Some("en-US"),
                        None,
                        7,
                    );
                }
                _ => unreachable!(),
            }
            assert_eq!(
                decode(&serde_json::to_string(&hostile).unwrap())
                    .unwrap_err()
                    .code(),
                ProblemCode::IntegrityFailed,
                "accepted {case}"
            );
        }
    }
}
