mod search_action_tests {
    use super::*;
    use crate::{
        BrowserRequestBoundaryPolicy, BrowserSessionAccessContext, BrowserSessionMutationCommand,
        NetworkClass, RequestAccessContext, SecretMaterial,
    };
    use fasti_domain::{CredentialId, OperationId};

    fn credential_access() -> ApplicationAccessContext {
        RequestAccessContext::new(
            WorkspaceId::new_v7(),
            ProfileId::new_v7(),
            ClientId::new_v7(),
            CredentialId::new_v7(),
            ProfileGrantId::new_v7(),
            1,
        )
        .into()
    }

    fn command() -> SearchCandidateActionCommand {
        SearchCandidateActionCommand {
            request: ReadSearchCandidateRequest {
                correlation_id: RequestCorrelationId::new_v7(),
                access: credential_access(),
                candidate_receipt_id: SearchCandidateReceiptId::new_v7(),
                provider: ProviderId::try_new("tmdb").unwrap(),
                grain: Grain::Film,
                outbound_policy: OutboundAccessPolicy::default(),
                terms_revision: "fasti.public-metadata-cache.v1".into(),
            },
            operation_id: OperationId::new_v7(),
            action: SearchRecordAction::Create,
            evidence_mode: SearchCandidateEvidenceMode::Refetch,
        }
    }

    #[test]
    fn action_digest_binds_exact_candidate_route_action_target_and_evidence_mode() {
        let original = command();
        let mutations: [fn(&mut SearchCandidateActionCommand); 5] = [
            |value| value.request.candidate_receipt_id = SearchCandidateReceiptId::new_v7(),
            |value| value.request.provider = ProviderId::try_new("google-books").unwrap(),
            |value| value.request.grain = Grain::Series,
            |value| value.action = SearchRecordAction::Attach(RecordId::new_v7()),
            |value| value.evidence_mode = SearchCandidateEvidenceMode::Cached,
        ];
        for mutate in mutations {
            let mut changed = original.clone();
            mutate(&mut changed);
            assert_ne!(original.semantic_digest(), changed.semantic_digest());
        }
        let mut attach = original;
        attach.action = SearchRecordAction::Attach(RecordId::new_v7());
        let mut changed_target = attach.clone();
        changed_target.action = SearchRecordAction::Attach(RecordId::new_v7());
        assert_ne!(attach.semantic_digest(), changed_target.semantic_digest());
        assert_eq!(attach.semantic_digest(), attach.clone().semantic_digest());
    }

    #[test]
    fn action_digest_excludes_tracing_operation_slot_and_current_execution_policy() {
        let original = command();
        let mutations: [fn(&mut SearchCandidateActionCommand); 5] = [
            |value| value.request.correlation_id = RequestCorrelationId::new_v7(),
            |value| value.operation_id = OperationId::new_v7(),
            |value| value.request.terms_revision = "fasti.public-metadata-cache.v2".into(),
            |value| {
                value.request.outbound_policy = OutboundAccessPolicy {
                    allow_providers: vec!["tmdb".into()],
                    deny_providers: vec!["google-books".into()],
                    allow_capabilities: vec!["metadata.read".into()],
                    deny_capabilities: vec!["metadata.search".into()],
                    allow_hosts: vec!["api.themoviedb.org".into()],
                    deny_hosts: vec!["www.googleapis.com".into()],
                    allow_networks: vec![NetworkClass::Public],
                    deny_networks: vec![NetworkClass::Loopback],
                };
            },
            // Actor/profile authorization is a separate mandatory replay check.
            // Digest equality alone must never grant access to another actor.
            |value| value.request.access = credential_access(),
        ];
        for mutate in mutations {
            let mut changed = original.clone();
            mutate(&mut changed);
            assert_eq!(original.semantic_digest(), changed.semantic_digest());
        }
        let ApplicationAccessContext::Credential(access) = &original.request.access else {
            panic!("credential fixture")
        };
        let mut rotated = original.clone();
        rotated.request.access = RequestAccessContext::new(
            access.workspace_id(),
            access.profile_id(),
            access.client_id(),
            CredentialId::new_v7(),
            access.grant_id(),
            access.presented_credential_epoch() + 1,
        )
        .into();
        assert_ne!(original.request.access, rotated.request.access);
        assert_eq!(original.semantic_digest(), rotated.semantic_digest());
    }

    fn browser_access(secret_byte: u8, seconds: i64) -> ApplicationAccessContext {
        let boundary =
            BrowserRequestBoundaryPolicy::try_new("https://fasti.example", "fasti.example")
                .unwrap();
        let proof = boundary
            .validate(Some("https://fasti.example"), Some("fasti.example"))
            .unwrap();
        BrowserSessionAccessContext::mutation(BrowserSessionMutationCommand::new(
            RequestCorrelationId::new_v7(),
            SecretMaterial::from_bytes([secret_byte; 32]),
            SecretMaterial::from_bytes([secret_byte.wrapping_add(1); 32]),
            proof,
            lifetime().created_at() + Duration::seconds(seconds),
        ))
        .into()
    }

    #[test]
    fn browser_session_rotation_and_request_time_do_not_change_action_intent() {
        let mut original = command();
        original.request.access = browser_access(7, 0);
        let mut rotated = original.clone();
        rotated.request.access = browser_access(8, 30);
        assert_ne!(original.request.access, rotated.request.access);
        assert_eq!(original.semantic_digest(), rotated.semantic_digest());
        // This pure test does not claim session validity or stable-subject
        // ownership: current authorization and stored actor checks own those.
    }

    #[test]
    fn action_and_evidence_mode_serialization_preserve_strict_existing_shapes() {
        let target = RecordId::new_v7();
        let create = serde_json::json!({"kind": "create"});
        let attach = serde_json::json!({"kind": "attach", "record_id": target});
        assert_eq!(
            serde_json::to_value(SearchRecordAction::Create).unwrap(),
            create
        );
        assert_eq!(
            serde_json::to_value(SearchRecordAction::Attach(target)).unwrap(),
            attach
        );
        assert_eq!(
            serde_json::from_value::<SearchRecordAction>(create).unwrap(),
            SearchRecordAction::Create
        );
        assert_eq!(
            serde_json::from_value::<SearchRecordAction>(attach.clone()).unwrap(),
            SearchRecordAction::Attach(target)
        );
        for invalid in [
            serde_json::json!({"kind": "create", "record_id": target}),
            serde_json::json!({"kind": "create", "extra": true}),
            serde_json::json!({"kind": "attach"}),
            serde_json::json!({"kind": "attach", "record_id": null}),
            serde_json::json!({"kind": "attach", "record_id": "not-a-record"}),
            serde_json::json!({"kind": "attach", "record_id": target, "extra": true}),
            serde_json::json!({"kind": "merge", "record_id": target}),
            serde_json::json!({"kind": "Create"}),
            serde_json::json!("create"),
            serde_json::json!(null),
        ] {
            assert!(
                serde_json::from_value::<SearchRecordAction>(invalid.clone()).is_err(),
                "{invalid}"
            );
        }
        for duplicate in [
            r#"{"kind":"create","kind":"create"}"#.to_owned(),
            format!(r#"{{"kind":"attach","record_id":"{target}","record_id":"{target}"}}"#),
        ] {
            assert!(serde_json::from_str::<SearchRecordAction>(&duplicate).is_err());
        }
        for (mode, name) in [
            (SearchCandidateEvidenceMode::Refetch, "refetch"),
            (SearchCandidateEvidenceMode::Cached, "cached"),
        ] {
            assert_eq!(serde_json::to_value(mode).unwrap(), serde_json::json!(name));
            assert_eq!(
                serde_json::from_value::<SearchCandidateEvidenceMode>(serde_json::json!(name))
                    .unwrap(),
                mode
            );
        }
        for invalid in [
            serde_json::json!("offline"),
            serde_json::json!("Cached"),
            serde_json::json!(null),
            serde_json::json!({"kind":"cached"}),
        ] {
            assert!(serde_json::from_value::<SearchCandidateEvidenceMode>(invalid).is_err());
        }
    }
}
