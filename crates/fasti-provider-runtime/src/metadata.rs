use crate::{ProviderRuntime, ProviderRuntimeError, ProviderSelectionInput};
use fasti_application::{
    metadata_field_group, CapabilityKey, CommitMetadataRefreshCommand,
    CommitMetadataRefreshReceiptCommand, FastiProblem, MarkMetadataRefreshUnavailableCommand,
    MetadataClaimRefreshService, MetadataRefreshFuture, MetadataRefreshMode,
    MetadataRefreshPersistencePort, OutboundAccessPolicy, PrepareMetadataRefreshCommand,
    ProblemCode, ProviderCapabilityId, ProviderCapabilityState, ProviderCapabilityStatus,
    ProviderId, ProviderMetadataField, ProviderOperationLease, ProviderStatePort,
    ProviderStatePortError, ReadCachedMetadataRefreshCommand, ReadMetadataRefreshReceiptCommand,
    RefreshMetadataClaimsCommand,
};
use fasti_domain::{
    MetadataAttribution, MetadataCacheEntry, MetadataCacheKey, MetadataCachePurpose,
    MetadataDataClassification, MetadataFieldGroup, MetadataLocale, MetadataRegion, NamespaceKey,
    ReceivedAt, Sha256Digest, METADATA_FRESH_SECONDS, METADATA_STALE_ON_ERROR_SECONDS,
    METADATA_STALE_WHILE_REFRESHING_SECONDS,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;

const READ_CAPABILITY: &str = "metadata.read";
const CACHE_SCHEMA_VERSION: u32 = 1;

/// Concrete refresh orchestration: authorize and resolve identity locally,
/// perform governed provider I/O without a database transaction, then
/// re-authorize and commit the immutable result atomically.
pub struct ProviderMetadataRefreshService {
    runtime: Arc<ProviderRuntime>,
    persistence: Arc<dyn MetadataRefreshPersistencePort>,
    provider_state: Arc<dyn ProviderStatePort>,
    outbound_policy: OutboundAccessPolicy,
}

impl ProviderMetadataRefreshService {
    pub fn new(
        runtime: Arc<ProviderRuntime>,
        persistence: Arc<dyn MetadataRefreshPersistencePort>,
        provider_state: Arc<dyn ProviderStatePort>,
        outbound_policy: OutboundAccessPolicy,
    ) -> Self {
        Self {
            runtime,
            persistence,
            provider_state,
            outbound_policy,
        }
    }

    async fn refresh(
        &self,
        command: RefreshMetadataClaimsCommand,
        lease: ProviderOperationLease,
    ) -> fasti_application::ApplicationResult<fasti_application::RefreshMetadataClaimsOutcome> {
        let capability = CapabilityKey::RefreshMetadataClaims;
        let correlation_id = command.correlation_id();
        validate_groups(command.field_groups(), capability, correlation_id)?;
        let provider_id = command.provider_id().clone();
        let semantic_digest = refresh_semantic_digest(&command, correlation_id)?;
        let persistence = Arc::clone(&self.persistence);
        let receipt = ReadMetadataRefreshReceiptCommand::new(
            correlation_id,
            *command.access(),
            command.operation_id(),
            semantic_digest.clone(),
            command.record_id(),
            provider_id.clone(),
        );
        if let Some(outcome) = run_blocking(&lease, capability, correlation_id, move || {
            persistence.authorize_and_read_refresh_receipt(receipt)
        })
        .await?
        {
            return Ok(outcome);
        }
        let persistence = Arc::clone(&self.persistence);
        let prepare = PrepareMetadataRefreshCommand::new(
            correlation_id,
            *command.access(),
            command.record_id(),
            provider_id.clone(),
            command.field_groups().to_vec(),
        );
        let prepared = run_blocking(&lease, capability, correlation_id, move || {
            persistence.authorize_and_prepare_refresh(prepare)
        })
        .await?;

        let provider = ProviderId::try_new(provider_id.as_str())
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?;
        let read_capability = ProviderCapabilityId::try_new(READ_CAPABILITY)
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?;
        let provider_state = Arc::clone(&self.provider_state);
        let workspace_id = command.access().workspace_id();
        let state = run_blocking(&lease, capability, correlation_id, move || {
            provider_state
                .get_provider_capability_state(workspace_id, &provider, &read_capability)
                .map_err(|error| state_problem(error, capability, correlation_id))?
                .ok_or_else(|| stale_problem(capability, correlation_id))
        })
        .await?;
        if matches!(
            state.capability_status(),
            ProviderCapabilityStatus::Disabled | ProviderCapabilityStatus::Unavailable
        ) {
            return Err(stale_problem(capability, correlation_id));
        }

        let mapping = fasti_application::provider_identity_mapping_for_grain(
            provider_id.as_str(),
            prepared.grain(),
        )
        .ok_or_else(|| problem(ProblemCode::ValidationFailed, capability, correlation_id))?;
        let descriptor = self
            .runtime
            .descriptor(provider_id.as_str())
            .map_err(|error| runtime_problem(error, capability, correlation_id))?;
        let effective_locale = provider_response_locale(
            provider_id.as_str(),
            command.locale(),
            capability,
            correlation_id,
        )?;
        let effective_region = provider_response_region(provider_id.as_str(), command.region());
        let enrichment_keys = cache_keys(
            &command,
            &prepared,
            &state,
            mapping.kind(),
            descriptor,
            MetadataCachePurpose::MetadataEnrichment,
            (effective_locale.clone(), effective_region.clone()),
        )?;
        let offline_keys = cache_keys(
            &command,
            &prepared,
            &state,
            mapping.kind(),
            descriptor,
            MetadataCachePurpose::OfflineRead,
            (effective_locale.clone(), effective_region.clone()),
        )?;
        if command.mode() == MetadataRefreshMode::PreferCache {
            let persistence = Arc::clone(&self.persistence);
            let cached = ReadCachedMetadataRefreshCommand::new(
                correlation_id,
                *command.access(),
                prepared.clone(),
                enrichment_keys.clone(),
            );
            if let Some(outcome) = run_blocking(&lease, capability, correlation_id, move || {
                persistence.authorize_and_read_cached_refresh(cached)
            })
            .await?
            {
                let persistence = Arc::clone(&self.persistence);
                let receipt = CommitMetadataRefreshReceiptCommand::new(
                    correlation_id,
                    *command.access(),
                    command.operation_id(),
                    semantic_digest.clone(),
                    command.record_id(),
                    provider_id.clone(),
                    outcome,
                );
                return run_blocking(&lease, capability, correlation_id, move || {
                    persistence.authorize_and_commit_refresh_receipt(receipt)
                })
                .await;
            }
        }
        let selection = ProviderSelectionInput {
            provider: provider_id.as_str().to_owned(),
            provider_id: prepared.identifier().value().to_owned(),
            kind: mapping.kind().to_owned(),
            locale: effective_locale
                .as_ref()
                .map(|value| value.as_str().to_owned()),
            region: effective_region
                .as_ref()
                .map(|value| value.as_str().to_owned()),
        };
        let candidate = match self
            .runtime
            .fetch_selection(selection, &self.outbound_policy, &state)
            .await
        {
            Ok(candidate) => candidate,
            Err(error) if error.problem_code() == ProblemCode::ProviderUnavailable => {
                let persistence = Arc::clone(&self.persistence);
                let unavailable = MarkMetadataRefreshUnavailableCommand::new(
                    correlation_id,
                    *command.access(),
                    prepared,
                    provider_id,
                );
                run_blocking(&lease, capability, correlation_id, move || {
                    persistence.authorize_and_mark_refresh_unavailable(unavailable)
                })
                .await?;
                return Err(stale_problem(capability, correlation_id));
            }
            Err(error) => return Err(fetch_problem(error, capability, correlation_id)),
        };
        if candidate.identifier().map_err(|_| {
            problem(
                ProblemCode::ProviderResponseInvalid,
                capability,
                correlation_id,
            )
        })? != *prepared.identifier()
        {
            return Err(problem(
                ProblemCode::ProviderResponseInvalid,
                capability,
                correlation_id,
            ));
        }

        let fields = candidate
            .metadata_fields(effective_locale.clone(), effective_region.clone())
            .map_err(|error| runtime_problem(error, capability, correlation_id))?
            .into_iter()
            .filter(|field| {
                metadata_field_group(field.field_key())
                    .is_some_and(|group| prepared.field_groups().contains(&group))
            })
            .collect::<Vec<_>>();
        let now = chrono::Utc::now();
        if !fields.iter().any(|field| {
            field.claim().initial_status() == fasti_domain::FieldClaimStatus::Fresh
                && field
                    .claim()
                    .expires_at()
                    .is_none_or(|expires_at| expires_at > now)
        }) {
            return Err(stale_problem(capability, correlation_id));
        }
        let attribution = MetadataAttribution::try_new(
            provider_id.clone(),
            descriptor.attribution,
            descriptor.docs_url,
        )
        .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?;
        let cache_entries = cache_entries(
            enrichment_keys.into_iter().chain(offline_keys),
            &fields,
            correlation_id,
        )?;

        let persistence = Arc::clone(&self.persistence);
        let commit = CommitMetadataRefreshCommand::new(
            correlation_id,
            *command.access(),
            command.operation_id(),
            semantic_digest,
            prepared,
            provider_id,
            state,
            fields,
            Vec::new(),
            cache_entries,
            attribution,
        );
        run_blocking(&lease, capability, correlation_id, move || {
            persistence.authorize_and_commit_refresh(commit)
        })
        .await
    }
}

fn refresh_semantic_digest(
    command: &RefreshMetadataClaimsCommand,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> fasti_application::ApplicationResult<Sha256Digest> {
    let capability = CapabilityKey::RefreshMetadataClaims;
    let mode = match command.mode() {
        MetadataRefreshMode::PreferCache => "prefer_cache",
        MetadataRefreshMode::Revalidate => "revalidate",
    };
    let encoded = serde_json::to_vec(&(
        command.record_id().to_string(),
        command.provider_id().as_str(),
        command.field_groups(),
        command.locale().map(MetadataLocale::as_str),
        command.region().map(MetadataRegion::as_str),
        mode,
    ))
    .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?;
    Ok(Sha256Digest::from_bytes(&Sha256::digest(encoded).into()))
}

pub(crate) async fn run_blocking<T, F>(
    lease: &ProviderOperationLease,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
    operation: F,
) -> fasti_application::ApplicationResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> fasti_application::ApplicationResult<T> + Send + 'static,
{
    let lease = lease.clone();
    tokio::task::spawn_blocking(move || {
        let _lease = lease;
        operation()
    })
    .await
    .map_err(|_| problem(ProblemCode::StorageUnavailable, capability, correlation_id))?
}

impl MetadataClaimRefreshService for ProviderMetadataRefreshService {
    fn authorize_and_refresh(
        &self,
        command: RefreshMetadataClaimsCommand,
        lease: ProviderOperationLease,
    ) -> MetadataRefreshFuture<'_> {
        Box::pin(self.refresh(command, lease))
    }
}

fn validate_groups(
    groups: &[MetadataFieldGroup],
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> fasti_application::ApplicationResult<()> {
    if groups.iter().all(|group| {
        matches!(
            group,
            MetadataFieldGroup::BasicInfo
                | MetadataFieldGroup::Artwork
                | MetadataFieldGroup::Details
                | MetadataFieldGroup::ReleaseDates
        )
    }) {
        Ok(())
    } else {
        Err(problem(
            ProblemCode::ValidationFailed,
            capability,
            correlation_id,
        ))
    }
}

fn cache_keys(
    command: &RefreshMetadataClaimsCommand,
    prepared: &fasti_application::PreparedMetadataRefresh,
    state: &ProviderCapabilityState,
    kind: &str,
    descriptor: &crate::ProviderSpec,
    purpose: MetadataCachePurpose,
    response_coordinates: (
        Option<fasti_domain::MetadataLocale>,
        Option<fasti_domain::MetadataRegion>,
    ),
) -> fasti_application::ApplicationResult<Vec<MetadataCacheKey>> {
    let capability = CapabilityKey::RefreshMetadataClaims;
    let correlation_id = command.correlation_id();
    let configuration_digest =
        Sha256Digest::parse(format!("sha256:{}", state.configuration_digest().as_str()))
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?;
    let credential_version = state
        .credential_reference()
        .map(|_| state.capability_version());
    let (locale, region) = response_coordinates;
    let mut keys = Vec::with_capacity(prepared.field_groups().len());
    for group in prepared.field_groups() {
        keys.push(
            MetadataCacheKey::try_new(
                command.provider_id().clone(),
                credential_version,
                command.record_id(),
                format!("metadata/{kind}"),
                prepared.grain(),
                NamespaceKey::try_new(prepared.identifier().namespace()).map_err(|_| {
                    problem(ProblemCode::IntegrityFailed, capability, correlation_id)
                })?,
                prepared.identifier().value(),
                locale.clone(),
                region.clone(),
                *group,
                prepared.settings_fingerprint().clone(),
                configuration_digest.clone(),
                CACHE_SCHEMA_VERSION,
                purpose,
                descriptor.cache_policy,
                MetadataDataClassification::Public,
            )
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))?,
        );
    }
    Ok(keys)
}

fn cache_entries(
    keys: impl Iterator<Item = MetadataCacheKey>,
    fields: &[ProviderMetadataField],
    correlation_id: fasti_domain::RequestCorrelationId,
) -> fasti_application::ApplicationResult<Vec<MetadataCacheEntry>> {
    let capability = CapabilityKey::RefreshMetadataClaims;
    let created = fields
        .first()
        .map(|field| ReceivedAt::from_application_clock(field.claim().fetched_at()))
        .expect("a successful refresh has at least one fresh claim");
    let fresh_until = created.value() + chrono::Duration::seconds(METADATA_FRESH_SECONDS);
    keys.map(|key| {
        let claim_ids = fields
            .iter()
            .filter(|field| metadata_field_group(field.field_key()) == Some(key.field_group()))
            .map(|field| field.claim().claim_id())
            .collect();
        MetadataCacheEntry::try_new(
            key,
            claim_ids,
            created,
            fresh_until,
            fresh_until + chrono::Duration::seconds(METADATA_STALE_WHILE_REFRESHING_SECONDS),
            created.value() + chrono::Duration::seconds(METADATA_STALE_ON_ERROR_SECONDS),
        )
        .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id))
    })
    .collect()
}

pub(crate) fn provider_response_locale(
    provider: &str,
    requested: Option<&fasti_domain::MetadataLocale>,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> fasti_application::ApplicationResult<Option<fasti_domain::MetadataLocale>> {
    match provider {
        crate::TMDB_PROVIDER => requested
            .cloned()
            .map_or_else(
                || fasti_domain::MetadataLocale::try_new("en-US").map(Some),
                |locale| Ok(Some(locale)),
            )
            .map_err(|_| problem(ProblemCode::IntegrityFailed, capability, correlation_id)),
        crate::GOOGLE_BOOKS_PROVIDER => Ok(None),
        _ => Ok(None),
    }
}

fn provider_response_region(
    provider: &str,
    requested: Option<&fasti_domain::MetadataRegion>,
) -> Option<fasti_domain::MetadataRegion> {
    (provider == crate::TMDB_PROVIDER)
        .then(|| requested.cloned())
        .flatten()
}

fn state_problem(
    error: ProviderStatePortError,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> Box<FastiProblem> {
    match error {
        ProviderStatePortError::Unavailable => {
            problem(ProblemCode::StorageUnavailable, capability, correlation_id)
        }
        ProviderStatePortError::Corrupt | ProviderStatePortError::RevisionConflict => {
            problem(ProblemCode::IntegrityFailed, capability, correlation_id)
        }
    }
}

fn runtime_problem(
    error: ProviderRuntimeError,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> Box<FastiProblem> {
    problem(error.problem_code(), capability, correlation_id)
}

fn fetch_problem(
    error: ProviderRuntimeError,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> Box<FastiProblem> {
    if error.problem_code() == ProblemCode::ProviderUnavailable {
        stale_problem(capability, correlation_id)
    } else {
        runtime_problem(error, capability, correlation_id)
    }
}

fn stale_problem(
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> Box<FastiProblem> {
    problem(ProblemCode::MetadataClaimStale, capability, correlation_id)
}

fn problem(
    code: ProblemCode,
    capability: CapabilityKey,
    correlation_id: fasti_domain::RequestCorrelationId,
) -> Box<FastiProblem> {
    Box::new(FastiProblem::from_code(code, capability, correlation_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone, Utc};
    use fasti_domain::{
        ClientId, CredentialId, ExternalIdentifierClaim, FieldClaim, FieldClaimProvenance,
        FieldClaimStatus, FieldKey, Grain, MetadataClaimId, MetadataLocale, MetadataProviderId,
        MetadataRegion, NamespaceKey, ProfileGrantId, ProfileId, RecordId, WorkspaceId,
    };

    #[tokio::test]
    async fn cancellation_retains_provider_lease_until_blocking_persistence_finishes() {
        let gate = Arc::new(tokio::sync::Mutex::new(()));
        let lease = ProviderOperationLease::new(Arc::clone(&gate).lock_owned().await);
        let (started, entered) = tokio::sync::oneshot::channel();
        let (finish, release) = std::sync::mpsc::channel();
        let caller = tokio::spawn(async move {
            run_blocking(
                &lease,
                CapabilityKey::RefreshMetadataClaims,
                fasti_domain::RequestCorrelationId::new_v7(),
                move || {
                    let _ = started.send(());
                    let _ = release.recv();
                    Ok(())
                },
            )
            .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(5), entered)
            .await
            .expect("bounded persistence start")
            .expect("entered persistence");
        caller.abort();
        assert!(caller.await.expect_err("cancel caller").is_cancelled());
        let held = gate.try_lock().is_err();
        finish.send(()).expect("release persistence");
        let _completed = tokio::time::timeout(std::time::Duration::from_secs(5), gate.lock())
            .await
            .expect("completed persistence releases gate");
        assert!(held);
    }

    fn test_field(at: chrono::DateTime<Utc>) -> ProviderMetadataField {
        let digest = Sha256Digest::parse(format!("sha256:{}", "a".repeat(64))).expect("digest");
        let provenance = FieldClaimProvenance::try_new(
            MetadataProviderId::try_new("tmdb").expect("provider"),
            NamespaceKey::try_new("tmdb.movie").expect("namespace"),
            "550",
            Some(MetadataLocale::try_new("en-US").expect("locale")),
            None,
            None,
            digest,
        )
        .expect("provenance");
        ProviderMetadataField::new(
            FieldKey::try_new(fasti_domain::TITLE_FIELD_KEY).expect("field"),
            FieldClaim::try_new_unbound_provider(
                MetadataClaimId::new_v7(),
                "Fight Club",
                provenance,
                ReceivedAt::from_application_clock(at),
                Some(at + Duration::seconds(METADATA_FRESH_SECONDS)),
                FieldClaimStatus::Fresh,
            )
            .expect("claim"),
        )
    }

    fn test_key(record_id: RecordId, purpose: MetadataCachePurpose) -> MetadataCacheKey {
        let digest = Sha256Digest::parse(format!("sha256:{}", "b".repeat(64))).expect("digest");
        MetadataCacheKey::try_new(
            MetadataProviderId::try_new("tmdb").expect("provider"),
            Some(1),
            record_id,
            "metadata/movie",
            Grain::Film,
            NamespaceKey::try_new("tmdb.movie").expect("namespace"),
            "550",
            Some(MetadataLocale::try_new("en-US").expect("locale")),
            None,
            MetadataFieldGroup::BasicInfo,
            digest.clone(),
            digest,
            1,
            purpose,
            "tmdb_attribution_required",
            MetadataDataClassification::Public,
        )
        .expect("cache key")
    }

    #[test]
    fn cache_windows_end_at_the_domain_caps_and_include_offline_partitions() {
        let at = Utc.timestamp_opt(1_800_000_000, 0).single().expect("time");
        let record_id = RecordId::new_v7();
        let entries = cache_entries(
            [
                test_key(record_id, MetadataCachePurpose::MetadataEnrichment),
                test_key(record_id, MetadataCachePurpose::OfflineRead),
            ]
            .into_iter(),
            &[test_field(at)],
            fasti_domain::RequestCorrelationId::new_v7(),
        )
        .expect("entries");

        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0].stale_on_error_until(),
            at + Duration::seconds(METADATA_STALE_ON_ERROR_SECONDS)
        );
        assert_eq!(
            entries[1].key().purpose(),
            MetadataCachePurpose::OfflineRead
        );
    }

    #[test]
    fn unsupported_groups_fail_before_transport_and_fetch_errors_keep_safe_detail() {
        let capability = CapabilityKey::RefreshMetadataClaims;
        let correlation_id = fasti_domain::RequestCorrelationId::new_v7();
        assert_eq!(
            validate_groups(&[MetadataFieldGroup::Credits], capability, correlation_id)
                .expect_err("unsupported")
                .code(),
            ProblemCode::ValidationFailed
        );
        assert_eq!(
            fetch_problem(
                ProviderRuntimeError::network("offline"),
                capability,
                correlation_id
            )
            .code(),
            ProblemCode::MetadataClaimStale
        );
        assert_eq!(
            fetch_problem(
                ProviderRuntimeError::credential_missing("missing"),
                capability,
                correlation_id
            )
            .code(),
            ProblemCode::ProviderCredentialMissing
        );
    }

    #[test]
    fn requested_locale_and_region_are_part_of_every_cache_partition() {
        let workspace_id = WorkspaceId::new_v7();
        let profile_id = ProfileId::new_v7();
        let record_id = RecordId::new_v7();
        let access = fasti_application::RequestAccessContext::new(
            workspace_id,
            profile_id,
            ClientId::new_v7(),
            CredentialId::new_v7(),
            ProfileGrantId::new_v7(),
            1,
        );
        let provider_id = MetadataProviderId::try_new("tmdb").expect("provider");
        let locale = MetadataLocale::try_new("fr-FR").expect("locale");
        let region = MetadataRegion::try_new("FR").expect("region");
        let command = RefreshMetadataClaimsCommand::new(
            fasti_domain::RequestCorrelationId::new_v7(),
            access,
            fasti_domain::OperationId::new_v7(),
            record_id,
            provider_id.clone(),
            vec![MetadataFieldGroup::BasicInfo],
            Some(locale.clone()),
            Some(region.clone()),
            MetadataRefreshMode::Revalidate,
        );
        let prepared = fasti_application::PreparedMetadataRefresh::new(
            record_id,
            Grain::Film,
            ExternalIdentifierClaim::try_new("tmdb.movie", Grain::Film, "438631")
                .expect("identifier"),
            vec![MetadataFieldGroup::BasicInfo],
            Sha256Digest::parse(format!("sha256:{}", "b".repeat(64))).expect("fingerprint"),
        );
        let state = fasti_application::ProviderCapabilityState::try_new(
            ProviderId::try_new("tmdb").expect("provider state ID"),
            ProviderCapabilityId::try_new("metadata.read").expect("capability ID"),
            ProviderCapabilityStatus::Available,
            4,
            fasti_application::CredentialRequirement::ApiKey,
            Some(fasti_application::CredentialReference::try_new("tmdb-api-key").expect("ref")),
            fasti_application::ProviderCredentialStatus::Valid,
            fasti_application::ConfigurationDigest::parse("c".repeat(64)).expect("config"),
            fasti_application::ProviderCheckMetadata::never_run(),
            fasti_application::ProviderCheckMetadata::never_run(),
        )
        .expect("provider state");

        let descriptor = crate::registry()
            .iter()
            .find(|entry| entry.provider == "tmdb")
            .unwrap();
        for purpose in [
            MetadataCachePurpose::MetadataEnrichment,
            MetadataCachePurpose::OfflineRead,
        ] {
            let keys_for = |descriptor| {
                cache_keys(
                    &command,
                    &prepared,
                    &state,
                    "movie",
                    descriptor,
                    purpose,
                    (Some(locale.clone()), Some(region.clone())),
                )
                .expect("cache keys")
            };
            let keys = keys_for(descriptor);
            assert!(keys.iter().all(|key| {
                key.locale().map(MetadataLocale::as_str) == Some("fr-fr")
                    && key.region().map(MetadataRegion::as_str) == Some("FR")
                    && key.purpose() == purpose
                    && key.terms_revision() == "fasti.public-metadata-cache.v1"
            }));
            let historical = crate::ProviderSpec {
                cache_policy: descriptor.licence_and_terms,
                ..*descriptor
            };
            assert_ne!(keys, keys_for(&historical));
        }
    }

    #[test]
    fn refresh_digest_uses_only_canonical_request_semantics() {
        let record_id = RecordId::new_v7();
        let provider_id = MetadataProviderId::try_new("tmdb").expect("provider");
        let locale = MetadataLocale::try_new("fr-FR").expect("locale");
        let region = MetadataRegion::try_new("FR").expect("region");
        let command = |mode| {
            RefreshMetadataClaimsCommand::new(
                fasti_domain::RequestCorrelationId::new_v7(),
                fasti_application::RequestAccessContext::new(
                    WorkspaceId::new_v7(),
                    ProfileId::new_v7(),
                    ClientId::new_v7(),
                    CredentialId::new_v7(),
                    ProfileGrantId::new_v7(),
                    7,
                ),
                fasti_domain::OperationId::new_v7(),
                record_id,
                provider_id.clone(),
                vec![MetadataFieldGroup::BasicInfo],
                Some(locale.clone()),
                Some(region.clone()),
                mode,
            )
        };
        let first = command(MetadataRefreshMode::PreferCache);
        let retry = command(MetadataRefreshMode::PreferCache);
        let revalidate = command(MetadataRefreshMode::Revalidate);

        assert_eq!(
            refresh_semantic_digest(&first, first.correlation_id()).expect("first digest"),
            refresh_semantic_digest(&retry, retry.correlation_id()).expect("retry digest")
        );
        assert_ne!(
            refresh_semantic_digest(&first, first.correlation_id()).expect("first digest"),
            refresh_semantic_digest(&revalidate, revalidate.correlation_id())
                .expect("revalidate digest")
        );
    }
}
