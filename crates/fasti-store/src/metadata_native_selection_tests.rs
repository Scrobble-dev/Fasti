mod metadata_native_selection_tests {
    use super::*;
    use fasti_application::GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY;

    fn observation(
        record: RecordId,
        value: &str,
        fetched: i64,
        status: FieldClaimStatus,
        reuse: ProviderResponseReuse,
    ) -> PolicyBoundFieldClaim {
        let at = received(fetched);
        let field = provider_field(
            GOOGLE_BOOKS_PROVIDER_ID,
            "googlebooks.volume",
            "42",
            GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY,
            value,
            at,
        );
        let expiry = (reuse != ProviderResponseReuse::ValidateEveryReuse)
            .then_some(received(fetched + 120).value());
        let claim = FieldClaim::try_new_provider(
            field.claim().claim_id(),
            record,
            field.field_key().clone(),
            value,
            field.claim().provenance().clone(),
            at,
            expiry,
            status,
        )
        .unwrap();
        let policy = ProviderResponseCachePolicy::new(
            reuse,
            at.value(),
            std::time::Duration::ZERO,
            Some(std::time::Duration::from_secs(120)),
            None,
        );
        (claim, Some(policy))
    }

    fn selected(claims: &[PolicyBoundFieldClaim], read_at: i64) -> Vec<FieldClaim> {
        let evidence: Vec<_> = claims
            .iter()
            .map(|(claim, policy)| MetadataReadEvidence {
                provenance: claim.provenance(),
                fetched_at: claim.fetched_at(),
                expires_at: claim.expires_at(),
                policy: *policy,
            })
            .collect();
        let mut allowed = reusable_metadata_evidence(&evidence, received(read_at).value());
        native::restrict_to_latest_observations(claims, &mut allowed, received(read_at).value());
        claims
            .iter()
            .zip(allowed)
            .filter_map(|((claim, _), allowed)| allowed.then_some(claim.clone()))
            .collect()
    }

    #[test]
    fn latest_negative_or_unknown_displaces_older_fresh_book_before_projection() {
        let record = RecordId::new_v7();
        let profile = ProfileId::new_v7();
        for value in ["MAGAZINE", "UNKNOWN"] {
            let mut old = observation(
                record,
                "BOOK",
                100,
                FieldClaimStatus::Fresh,
                ProviderResponseReuse::Reusable,
            );
            // Historical policy absence must not allow a fresh older positive
            // to outrank a newer stale but reusable negative observation.
            old.1 = None;
            let latest = observation(
                record,
                value,
                150,
                FieldClaimStatus::Stale,
                ProviderResponseReuse::Reusable,
            );
            for claims in [
                vec![old.clone(), latest.clone()],
                vec![latest.clone(), old.clone()],
            ] {
                let admitted = selected(&claims, 200);
                assert_eq!(admitted, vec![latest.0.clone()]);
                let resolved = resolve_profile_field(
                    None,
                    &admitted,
                    &[],
                    &MetadataProjectionPolicy::default_for_profile(profile),
                    received(200).value(),
                )
                .unwrap();
                assert_eq!(resolved.value(), Some(value));
                assert!(resolved.is_stale());
            }
        }
    }

    #[test]
    fn denied_or_terminal_latest_observation_never_resurrects_older_book() {
        let record = RecordId::new_v7();
        for (status, reuse) in [
            (FieldClaimStatus::Invalid, ProviderResponseReuse::Reusable),
            (FieldClaimStatus::Revoked, ProviderResponseReuse::Reusable),
            (
                FieldClaimStatus::Superseded,
                ProviderResponseReuse::Reusable,
            ),
            (
                FieldClaimStatus::Stale,
                ProviderResponseReuse::ValidateEveryReuse,
            ),
        ] {
            let mut old = observation(
                record,
                "BOOK",
                100,
                FieldClaimStatus::Fresh,
                ProviderResponseReuse::Reusable,
            );
            old.1 = None;
            let latest = observation(record, "BOOK", 150, status, reuse);
            for claims in [
                vec![old.clone(), latest.clone()],
                vec![latest.clone(), old.clone()],
            ] {
                assert!(selected(&claims, 200).is_empty(), "{status:?}/{reuse:?}");
            }
        }
    }

    #[test]
    fn equal_time_conflicting_values_or_denial_are_order_independent() {
        let record = RecordId::new_v7();
        let book = observation(
            record,
            "BOOK",
            100,
            FieldClaimStatus::Fresh,
            ProviderResponseReuse::Reusable,
        );
        for (value, status, reuse) in [
            (
                "MAGAZINE",
                FieldClaimStatus::Fresh,
                ProviderResponseReuse::Reusable,
            ),
            (
                "UNKNOWN",
                FieldClaimStatus::Fresh,
                ProviderResponseReuse::Reusable,
            ),
            (
                "BOOK",
                FieldClaimStatus::Stale,
                ProviderResponseReuse::ValidateEveryReuse,
            ),
        ] {
            let other = observation(record, value, 100, status, reuse);
            // Store's unique observation key rejects equal-time conflicting
            // rows. Exercise the pure selector without bypassing that key.
            for claims in [
                vec![book.clone(), other.clone()],
                vec![other.clone(), book.clone()],
            ] {
                assert!(selected(&claims, 150).is_empty());
            }
        }
        let same = observation(
            record,
            "BOOK",
            100,
            FieldClaimStatus::Fresh,
            ProviderResponseReuse::Reusable,
        );
        assert_eq!(selected(&[book, same], 150).len(), 2);
    }

    #[test]
    fn latest_consistent_stale_book_respects_profile_last_known_good_policy() {
        let record = RecordId::new_v7();
        let profile = ProfileId::new_v7();
        for status in [FieldClaimStatus::Stale, FieldClaimStatus::Unavailable] {
            let old = observation(
                record,
                "BOOK",
                100,
                FieldClaimStatus::Fresh,
                ProviderResponseReuse::Reusable,
            );
            let latest = observation(record, "BOOK", 150, status, ProviderResponseReuse::Reusable);
            let admitted = selected(&[old, latest.clone()], 200);
            assert_eq!(admitted, vec![latest.0]);
            for last_known_good in [LastKnownGoodPolicy::Allow, LastKnownGoodPolicy::Deny] {
                let policy = MetadataProjectionPolicy::new(
                    profile,
                    None,
                    None,
                    None,
                    false,
                    last_known_good,
                );
                let resolved =
                    resolve_profile_field(None, &admitted, &[], &policy, received(200).value())
                        .unwrap();
                if last_known_good == LastKnownGoodPolicy::Allow {
                    assert_eq!(resolved.value(), Some("BOOK"));
                    assert!(resolved.is_stale());
                } else {
                    assert_eq!(resolved.value(), None);
                }
            }
        }
    }

    #[test]
    fn one_record_read_applies_latest_value_policy_and_appended_lifecycle() {
        for (value, status, reuse, expected) in [
            (
                "MAGAZINE",
                FieldClaimStatus::Fresh,
                ProviderResponseReuse::Reusable,
                Some("MAGAZINE"),
            ),
            (
                "UNKNOWN",
                FieldClaimStatus::Fresh,
                ProviderResponseReuse::Reusable,
                Some("UNKNOWN"),
            ),
            (
                "BOOK",
                FieldClaimStatus::Invalid,
                ProviderResponseReuse::Reusable,
                None,
            ),
            (
                "BOOK",
                FieldClaimStatus::Revoked,
                ProviderResponseReuse::Reusable,
                None,
            ),
            (
                "BOOK",
                FieldClaimStatus::Superseded,
                ProviderResponseReuse::Reusable,
                None,
            ),
            (
                "BOOK",
                FieldClaimStatus::Stale,
                ProviderResponseReuse::ValidateEveryReuse,
                None,
            ),
        ] {
            let node = TestNode::new();
            let record = node
                .kernel
                .create_record(CreateRecordCommand::new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    Grain::Edition,
                ))
                .unwrap()
                .record_id();
            let old = observation(
                record,
                "BOOK",
                100,
                FieldClaimStatus::Fresh,
                ProviderResponseReuse::Reusable,
            );
            let latest = observation(
                record,
                value,
                150,
                if reuse == ProviderResponseReuse::ValidateEveryReuse {
                    FieldClaimStatus::Stale
                } else {
                    FieldClaimStatus::Fresh
                },
                reuse,
            );
            let key = field_key(GOOGLE_BOOKS_PRINT_TYPE_FIELD_KEY);
            let connection = node.kernel.inner.connection.lock().unwrap();
            let capability = CapabilityKey::ReadMetadataProjection;
            let correlation = RequestCorrelationId::new_v7();
            for (claim, policy) in [&old, &latest] {
                let policy_json = policy.unwrap().to_canonical_json();
                write_field_claim(
                    &connection,
                    node.access.workspace_id(),
                    record,
                    &key,
                    claim,
                    capability,
                    correlation,
                    Some(&policy_json),
                )
                .unwrap();
            }
            if matches!(
                status,
                FieldClaimStatus::Invalid
                    | FieldClaimStatus::Revoked
                    | FieldClaimStatus::Superseded
            ) {
                let event = FieldClaimLifecycleEvent::try_new(
                    latest.0.claim_id(),
                    1,
                    FieldClaimStatus::Fresh,
                    status,
                    received(175),
                    Some(digest("f")),
                )
                .unwrap();
                append_field_claim_lifecycle_event(
                    &connection,
                    node.access.workspace_id(),
                    &event,
                    capability,
                    correlation,
                )
                .unwrap();
            }
            let admitted = load_field_claims(
                &connection,
                node.access.workspace_id(),
                record,
                &key,
                capability,
                correlation,
                received(200).value(),
            )
            .unwrap();
            assert_eq!(admitted.len(), usize::from(expected.is_some()));
            assert_eq!(admitted.first().map(FieldClaim::value), expected);
            if let Some(claim) = admitted.first() {
                assert_eq!(claim.claim_id(), latest.0.claim_id());
                assert_eq!(claim.fetched_at(), received(150).value());
                assert_eq!(claim.provenance(), latest.0.provenance());
            }
            assert_eq!(connection.query_row(
                "SELECT COUNT(*) FROM metadata_field_claims WHERE record_id = ?1 AND field_key = ?2",
                params![record.to_string(), key.as_str()], |row| row.get::<_, i64>(0),
            ).unwrap(), 2, "selection must not delete history");
        }
    }
}
