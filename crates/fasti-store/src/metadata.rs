//! Persistence for provider metadata claims and user overrides.
//!
//! `fasti_domain::metadata` models `FieldClaim`/`FieldOverride` and the
//! deterministic `resolve_field()` tiering, but had zero SQLite persistence.
//! These functions are the store-side half: write every claim a provider
//! ever supplied (history, never overwritten in place) and the single
//! current override per field, then read them back for resolution.

use crate::identity::{attach_identifier_tx, insert_record, matching_record_ids};
use crate::kernel::{
    authorize_transaction, map_sql, now, parse_timestamp, timestamp, SqliteKernel,
};
use fasti_application::{
    metadata_field_group, provider_identity_mapping_for_grain, ApplicationResult,
    ApplyProviderMetadataCommand, CapabilityKey, CommitMetadataRefreshCommand,
    ConfigureMetadataProjectionCommand, ConfigureMetadataProjectionOutcome,
    CreateProviderRecordCommand, CreateProviderRecordOutcome, FastiProblem,
    MarkMetadataRefreshUnavailableCommand, MetadataCacheReadView, MetadataOverrideMutation,
    MetadataProjectionPort, MetadataProjectionView, MetadataRefreshPersistencePort,
    PrepareMetadataRefreshCommand, PreparedMetadataRefresh, ProblemCode, ProviderCapabilityState,
    ProviderMetadataField, ProviderMetadataPort, RatingClaimView, ReadCachedMetadataRefreshCommand,
    ReadMetadataProjectionQuery, RefreshMetadataClaimsOutcome, MAX_PROVIDER_METADATA_FIELDS,
};
use fasti_domain::{
    resolve_profile_field, EnrichmentPolicy, FieldClaim, FieldClaimError, FieldClaimLifecycleEvent,
    FieldClaimProvenance, FieldClaimStatus, FieldKey, FieldOverride, FieldResolutionTier,
    LastKnownGoodPolicy, MetadataAttribution, MetadataCacheEntry, MetadataCacheInvalidationReason,
    MetadataCacheKey, MetadataCachePurpose, MetadataCacheReadState, MetadataClaimId,
    MetadataDataClassification, MetadataFieldGroup, MetadataLocale, MetadataProjection,
    MetadataProjectionPolicy, MetadataProviderId, MetadataRegion, NamespaceKey,
    ProfileFieldOverride, ProfileId, RatingClaim, RatingScale, ReceivedAt, RecordId,
    RequestCorrelationId, Sha256Digest, WorkspaceId,
};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};
use std::collections::{BTreeSet, HashMap};

const MAX_EFFECTIVE_FIELD_CLAIMS: i64 = 256;

fn claim_status(value: FieldClaimStatus) -> &'static str {
    match value {
        FieldClaimStatus::Fresh => "fresh",
        FieldClaimStatus::Stale => "stale",
        FieldClaimStatus::Invalid => "invalid",
        FieldClaimStatus::Revoked => "revoked",
        FieldClaimStatus::Superseded => "superseded",
        FieldClaimStatus::Unavailable => "unavailable",
    }
}

fn parse_claim_status(value: &str) -> Option<FieldClaimStatus> {
    match value {
        "fresh" => Some(FieldClaimStatus::Fresh),
        "stale" => Some(FieldClaimStatus::Stale),
        "invalid" => Some(FieldClaimStatus::Invalid),
        "revoked" => Some(FieldClaimStatus::Revoked),
        "superseded" => Some(FieldClaimStatus::Superseded),
        "unavailable" => Some(FieldClaimStatus::Unavailable),
        _ => None,
    }
}

fn field_group(value: MetadataFieldGroup) -> &'static str {
    match value {
        MetadataFieldGroup::Artwork => "artwork",
        MetadataFieldGroup::BasicInfo => "basic_info",
        MetadataFieldGroup::Details => "details",
        MetadataFieldGroup::ReleaseDates => "release_dates",
        MetadataFieldGroup::Credits => "credits",
        MetadataFieldGroup::ProductionCompanies => "production_companies",
        MetadataFieldGroup::Networks => "networks",
        MetadataFieldGroup::Episodes => "episodes",
        MetadataFieldGroup::SeasonArtwork => "season_artwork",
        MetadataFieldGroup::Recommendations => "recommendations",
        MetadataFieldGroup::Collections => "collections",
        MetadataFieldGroup::Trailers => "trailers",
        MetadataFieldGroup::WatchProviders => "watch_providers",
    }
}

fn parse_field_group(value: &str) -> Option<MetadataFieldGroup> {
    MetadataFieldGroup::ALL
        .iter()
        .copied()
        .find(|candidate| field_group(*candidate) == value)
}

fn encode_field_groups(groups: &[MetadataFieldGroup]) -> Result<String, serde_json::Error> {
    serde_json::to_string(&groups.iter().copied().map(field_group).collect::<Vec<_>>())
}

fn decode_field_groups(value: &str) -> Result<Vec<MetadataFieldGroup>, ()> {
    let encoded = serde_json::from_str::<Vec<String>>(value).map_err(|_| ())?;
    encoded
        .iter()
        .map(|value| parse_field_group(value).ok_or(()))
        .collect()
}

fn resolution_tier(value: FieldResolutionTier) -> &'static str {
    match value {
        FieldResolutionTier::UserOverride => "user_override",
        FieldResolutionTier::PreferredProviderClaim => "preferred_provider_claim",
        FieldResolutionTier::FallbackProviderClaim => "fallback_provider_claim",
        FieldResolutionTier::LastKnownGood => "last_known_good",
        FieldResolutionTier::Empty => "empty",
    }
}

fn enrichment_policy_fingerprint(
    policy: &EnrichmentPolicy,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<Sha256Digest> {
    let encoded = serde_json::to_vec(policy)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    format!("sha256:{}", crate::crypto::sha256_hex(&encoded))
        .parse()
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))
}

fn cache_purpose(value: MetadataCachePurpose) -> &'static str {
    match value {
        MetadataCachePurpose::MetadataEnrichment => "metadata_enrichment",
        MetadataCachePurpose::DisplayProjection => "display_projection",
        MetadataCachePurpose::RatingLookup => "rating_lookup",
        MetadataCachePurpose::OfflineRead => "offline_read",
    }
}

fn parse_cache_purpose(value: &str) -> Option<MetadataCachePurpose> {
    match value {
        "metadata_enrichment" => Some(MetadataCachePurpose::MetadataEnrichment),
        "display_projection" => Some(MetadataCachePurpose::DisplayProjection),
        "rating_lookup" => Some(MetadataCachePurpose::RatingLookup),
        "offline_read" => Some(MetadataCachePurpose::OfflineRead),
        _ => None,
    }
}

fn data_classification(value: MetadataDataClassification) -> &'static str {
    match value {
        MetadataDataClassification::Public => "public",
        MetadataDataClassification::Internal => "internal",
        MetadataDataClassification::Confidential => "confidential",
        MetadataDataClassification::Restricted => "restricted",
    }
}

fn parse_data_classification(value: &str) -> Option<MetadataDataClassification> {
    match value {
        "public" => Some(MetadataDataClassification::Public),
        "internal" => Some(MetadataDataClassification::Internal),
        "confidential" => Some(MetadataDataClassification::Confidential),
        "restricted" => Some(MetadataDataClassification::Restricted),
        _ => None,
    }
}

fn invalidation_reason(value: MetadataCacheInvalidationReason) -> &'static str {
    match value {
        MetadataCacheInvalidationReason::ProviderConfigurationChanged => {
            "provider_configuration_changed"
        }
        MetadataCacheInvalidationReason::CredentialRotated => "credential_rotated",
        MetadataCacheInvalidationReason::ProjectionPolicyChanged => "projection_policy_changed",
        MetadataCacheInvalidationReason::TermsChanged => "terms_changed",
        MetadataCacheInvalidationReason::ExplicitRetraction => "explicit_retraction",
    }
}

fn parse_invalidation_reason(value: &str) -> Option<MetadataCacheInvalidationReason> {
    match value {
        "provider_configuration_changed" => {
            Some(MetadataCacheInvalidationReason::ProviderConfigurationChanged)
        }
        "credential_rotated" => Some(MetadataCacheInvalidationReason::CredentialRotated),
        "projection_policy_changed" => {
            Some(MetadataCacheInvalidationReason::ProjectionPolicyChanged)
        }
        "terms_changed" => Some(MetadataCacheInvalidationReason::TermsChanged),
        "explicit_retraction" => Some(MetadataCacheInvalidationReason::ExplicitRetraction),
        _ => None,
    }
}

fn cache_storage_key(key: &MetadataCacheKey) -> Result<String, serde_json::Error> {
    serde_json::to_vec(key).map(|encoded| crate::crypto::sha256_hex(&encoded))
}

pub(crate) fn write_field_claim(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    field_key: &FieldKey,
    claim: &FieldClaim,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    if claim.record_id().is_some_and(|value| value != record_id)
        || claim.field_key().is_some_and(|value| value != field_key)
    {
        return Err(invalid_provider_metadata(capability, correlation_id));
    }
    map_sql(
        connection.execute_batch("SAVEPOINT metadata_claim_write"),
        capability,
        correlation_id,
    )?;
    let result = write_field_claim_inner(
        connection,
        workspace_id,
        record_id,
        field_key,
        claim,
        capability,
        correlation_id,
    );
    match result {
        Ok(()) => map_sql(
            connection.execute_batch("RELEASE metadata_claim_write"),
            capability,
            correlation_id,
        ),
        Err(error) => {
            let _ = connection
                .execute_batch("ROLLBACK TO metadata_claim_write; RELEASE metadata_claim_write");
            Err(error)
        }
    }
}

fn write_field_claim_inner(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    field_key: &FieldKey,
    claim: &FieldClaim,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    let inserted = map_sql(
        connection.execute(
            r#"
            INSERT INTO metadata_field_claims(
                workspace_id, record_id, field_key, source, value, locale,
                fetched_at, expires_at, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(record_id, field_key, source, fetched_at) DO NOTHING
            "#,
            params![
                workspace_id.to_string(),
                record_id.to_string(),
                field_key.as_str(),
                claim.source().as_str(),
                claim.value(),
                claim.locale(),
                timestamp(claim.fetched_at()),
                claim.expires_at().map(timestamp),
                timestamp(now())
            ],
        ),
        capability,
        correlation_id,
    )?;
    if inserted == 0 {
        return verify_identical_field_claim(
            connection,
            workspace_id,
            record_id,
            field_key,
            claim,
            capability,
            correlation_id,
        );
    }

    let provenance = claim.provenance();
    map_sql(
        connection.execute(
            r#"
            INSERT INTO metadata_claims(
                claim_id, workspace_id, record_id, claim_kind, created_at
            ) VALUES (?1, ?2, ?3, 'field', ?4)
            "#,
            params![
                claim.claim_id().to_string(),
                workspace_id.to_string(),
                record_id.to_string(),
                timestamp(now())
            ],
        ),
        capability,
        correlation_id,
    )?;
    map_sql(
        connection.execute(
            r#"
            INSERT INTO metadata_claim_provenance(
                claim_id, workspace_id, record_id, field_key, source, fetched_at,
                provider_id, source_record_id, region, source_version,
                evidence_digest, classification, terms_revision,
                provenance_state, initial_status, created_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                'internal', NULL, ?12, ?13, ?14
            )
            "#,
            params![
                claim.claim_id().to_string(),
                workspace_id.to_string(),
                record_id.to_string(),
                field_key.as_str(),
                claim.source().as_str(),
                timestamp(claim.fetched_at()),
                provenance.provider_id().map(|value| value.as_str()),
                provenance.source_identifier(),
                provenance.region().map(|value| value.as_str()),
                provenance.source_version(),
                provenance.evidence_digest().map(ToString::to_string),
                if provenance.is_complete() {
                    "complete"
                } else {
                    "legacy_incomplete"
                },
                claim_status(claim.initial_status()),
                timestamp(now())
            ],
        ),
        capability,
        correlation_id,
    )?;
    Ok(())
}

fn verify_identical_field_claim(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    field_key: &FieldKey,
    claim: &FieldClaim,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    let existing = map_sql(
        connection
            .query_row(
                r#"
                SELECT claim.value, claim.locale, claim.expires_at,
                       provenance.provider_id, provenance.source_record_id,
                       provenance.region, provenance.source_version,
                       provenance.evidence_digest, provenance.provenance_state,
                       provenance.initial_status
                FROM metadata_field_claims claim
                JOIN metadata_claim_provenance provenance
                  ON provenance.record_id = claim.record_id
                 AND provenance.field_key = claim.field_key
                 AND provenance.source = claim.source
                 AND provenance.fetched_at = claim.fetched_at
                WHERE claim.workspace_id = ?1 AND claim.record_id = ?2
                  AND claim.field_key = ?3 AND claim.source = ?4
                  AND claim.fetched_at = ?5
                "#,
                params![
                    workspace_id.to_string(),
                    record_id.to_string(),
                    field_key.as_str(),
                    claim.source().as_str(),
                    timestamp(claim.fetched_at())
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                    ))
                },
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    let provenance = claim.provenance();
    let expected = (
        claim.value().to_owned(),
        claim.locale().map(ToOwned::to_owned),
        claim.expires_at().map(timestamp),
        provenance
            .provider_id()
            .map(|value| value.as_str().to_owned()),
        provenance.source_identifier().map(ToOwned::to_owned),
        provenance.region().map(|value| value.as_str().to_owned()),
        provenance.source_version().map(ToOwned::to_owned),
        provenance.evidence_digest().map(ToString::to_string),
        if provenance.is_complete() {
            "complete".to_owned()
        } else {
            "legacy_incomplete".to_owned()
        },
        claim_status(claim.initial_status()).to_owned(),
    );
    if existing.as_ref() == Some(&expected) {
        Ok(())
    } else {
        Err(immutable_claim_conflict(capability, correlation_id))
    }
}

pub(crate) fn append_field_claim_lifecycle_event(
    connection: &Connection,
    workspace_id: WorkspaceId,
    event: &FieldClaimLifecycleEvent,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    let latest = map_sql(
        connection
            .query_row(
                r#"
                SELECT COALESCE(MAX(lifecycle.sequence), 0),
                       COALESCE((
                           SELECT current.status
                           FROM metadata_claim_lifecycle_events current
                           WHERE current.claim_id = registered.claim_id
                           ORDER BY current.sequence DESC LIMIT 1
                       ), CASE registered.claim_kind
                            WHEN 'field' THEN (
                                SELECT provenance.initial_status
                                FROM metadata_claim_provenance provenance
                                WHERE provenance.claim_id = registered.claim_id
                            )
                            WHEN 'rating' THEN (
                                SELECT rating.initial_status
                                FROM metadata_rating_claims rating
                                WHERE rating.claim_id = registered.claim_id
                            )
                        END)
                FROM metadata_claims registered
                LEFT JOIN metadata_claim_lifecycle_events lifecycle
                  ON lifecycle.claim_id = registered.claim_id
                WHERE registered.claim_id = ?1 AND registered.workspace_id = ?2
                GROUP BY registered.claim_id, registered.claim_kind
                "#,
                params![event.claim_id().to_string(), workspace_id.to_string()],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    let Some((latest_sequence, latest_status)) = latest else {
        return Err(immutable_claim_conflict(capability, correlation_id));
    };
    let expected_sequence = i64::from(event.sequence());
    if latest_sequence + 1 != expected_sequence
        || parse_claim_status(&latest_status) != Some(event.previous_status())
    {
        let existing = map_sql(
            connection
                .query_row(
                    r#"
                    SELECT previous_status, status, occurred_at, evidence_digest
                    FROM metadata_claim_lifecycle_events
                    WHERE claim_id = ?1 AND sequence = ?2
                    "#,
                    params![event.claim_id().to_string(), expected_sequence],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, Option<String>>(3)?,
                        ))
                    },
                )
                .optional(),
            capability,
            correlation_id,
        )?;
        let expected = (
            claim_status(event.previous_status()).to_owned(),
            claim_status(event.status()).to_owned(),
            timestamp(event.occurred_at()),
            event.evidence_digest().map(ToString::to_string),
        );
        return if existing.as_ref() == Some(&expected) {
            Ok(())
        } else {
            Err(immutable_claim_conflict(capability, correlation_id))
        };
    }
    map_sql(
        connection.execute(
            r#"
            INSERT INTO metadata_claim_lifecycle_events(
                claim_id, sequence, workspace_id, previous_status, status,
                occurred_at, evidence_digest
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                event.claim_id().to_string(),
                expected_sequence,
                workspace_id.to_string(),
                claim_status(event.previous_status()),
                claim_status(event.status()),
                timestamp(event.occurred_at()),
                event.evidence_digest().map(ToString::to_string)
            ],
        ),
        capability,
        correlation_id,
    )?;
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn write_rating_claim(
    connection: &Connection,
    workspace_id: WorkspaceId,
    claim: &RatingClaim,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    map_sql(
        connection.execute_batch("SAVEPOINT metadata_rating_claim_write"),
        capability,
        correlation_id,
    )?;
    let result =
        write_rating_claim_inner(connection, workspace_id, claim, capability, correlation_id);
    match result {
        Ok(()) => map_sql(
            connection.execute_batch("RELEASE metadata_rating_claim_write"),
            capability,
            correlation_id,
        ),
        Err(error) => {
            let _ = connection.execute_batch(
                "ROLLBACK TO metadata_rating_claim_write; RELEASE metadata_rating_claim_write",
            );
            Err(error)
        }
    }
}

fn write_rating_claim_inner(
    connection: &Connection,
    workspace_id: WorkspaceId,
    claim: &RatingClaim,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    let inserted = map_sql(
        connection.execute(
            r#"
            INSERT INTO metadata_claims(
                claim_id, workspace_id, record_id, claim_kind, created_at
            ) VALUES (?1, ?2, ?3, 'rating', ?4)
            ON CONFLICT(claim_id) DO NOTHING
            "#,
            params![
                claim.claim_id().to_string(),
                workspace_id.to_string(),
                claim.record_id().to_string(),
                timestamp(now())
            ],
        ),
        capability,
        correlation_id,
    )?;
    if inserted == 0 {
        return verify_identical_rating_claim(
            connection,
            workspace_id,
            claim,
            capability,
            correlation_id,
        );
    }
    let provenance = claim.provenance();
    let provider_id = provenance
        .provider_id()
        .ok_or_else(|| invalid_provider_metadata(capability, correlation_id))?;
    let source_record_id = provenance
        .source_identifier()
        .ok_or_else(|| invalid_provider_metadata(capability, correlation_id))?;
    let evidence_digest = provenance
        .evidence_digest()
        .ok_or_else(|| invalid_provider_metadata(capability, correlation_id))?;
    map_sql(
        connection.execute(
            r#"
            INSERT INTO metadata_rating_claims(
                claim_id, workspace_id, record_id, value_millis,
                scale_minimum_millis, scale_maximum_millis, provider_id,
                source, source_record_id, locale, region, source_version,
                evidence_digest, classification, terms_revision, fetched_at,
                expires_at, initial_status, created_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, 'internal', NULL, ?14, ?15, ?16, ?17
            )
            "#,
            params![
                claim.claim_id().to_string(),
                workspace_id.to_string(),
                claim.record_id().to_string(),
                i64::from(claim.value_millis()),
                i64::from(claim.scale().minimum_millis()),
                i64::from(claim.scale().maximum_millis()),
                provider_id.as_str(),
                provenance.source_namespace().as_str(),
                source_record_id,
                provenance.locale().map(|value| value.as_str()),
                provenance.region().map(|value| value.as_str()),
                provenance.source_version(),
                evidence_digest.to_string(),
                timestamp(claim.fetched_at()),
                claim.expires_at().map(timestamp),
                claim_status(claim.initial_status()),
                timestamp(now())
            ],
        ),
        capability,
        correlation_id,
    )?;
    Ok(())
}

fn verify_identical_rating_claim(
    connection: &Connection,
    workspace_id: WorkspaceId,
    claim: &RatingClaim,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    let existing = map_sql(
        connection
            .query_row(
                r#"
                SELECT registered.record_id, rating.value_millis,
                       rating.scale_minimum_millis, rating.scale_maximum_millis,
                       rating.provider_id, rating.source, rating.source_record_id,
                       rating.locale, rating.region, rating.source_version,
                       rating.evidence_digest, rating.fetched_at, rating.expires_at,
                       rating.initial_status
                FROM metadata_claims registered
                JOIN metadata_rating_claims rating ON rating.claim_id = registered.claim_id
                WHERE registered.claim_id = ?1
                  AND registered.workspace_id = ?2
                  AND registered.claim_kind = 'rating'
                "#,
                params![claim.claim_id().to_string(), workspace_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, Option<String>>(9)?,
                        row.get::<_, String>(10)?,
                        row.get::<_, String>(11)?,
                        row.get::<_, Option<String>>(12)?,
                        row.get::<_, String>(13)?,
                    ))
                },
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    let provenance = claim.provenance();
    let matches_existing = existing.is_some_and(
        |(
            record_id,
            value_millis,
            scale_minimum,
            scale_maximum,
            provider_id,
            source,
            source_record_id,
            locale,
            region,
            source_version,
            evidence_digest,
            fetched_at,
            expires_at,
            initial_status,
        )| {
            record_id == claim.record_id().to_string()
                && value_millis == i64::from(claim.value_millis())
                && scale_minimum == i64::from(claim.scale().minimum_millis())
                && scale_maximum == i64::from(claim.scale().maximum_millis())
                && provider_id
                    == provenance
                        .provider_id()
                        .map(|value| value.as_str().to_owned())
                        .unwrap_or_default()
                && source == provenance.source_namespace().as_str()
                && source_record_id == provenance.source_identifier().unwrap_or_default()
                && locale.as_deref() == provenance.locale().map(MetadataLocale::as_str)
                && region.as_deref() == provenance.region().map(MetadataRegion::as_str)
                && source_version.as_deref() == provenance.source_version()
                && evidence_digest
                    == provenance
                        .evidence_digest()
                        .map(ToString::to_string)
                        .unwrap_or_default()
                && fetched_at == timestamp(claim.fetched_at())
                && expires_at == claim.expires_at().map(timestamp)
                && initial_status == claim_status(claim.initial_status())
        },
    );
    if matches_existing {
        Ok(())
    } else {
        Err(immutable_claim_conflict(capability, correlation_id))
    }
}

#[allow(dead_code)]
pub(crate) fn load_rating_claims(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<Vec<RatingClaim>> {
    let mut statement = map_sql(
        connection.prepare(
            r#"
            SELECT rating.claim_id, rating.value_millis,
                   rating.scale_minimum_millis, rating.scale_maximum_millis,
                   rating.provider_id, rating.source, rating.source_record_id,
                   rating.locale, rating.region, rating.source_version,
                   rating.evidence_digest, rating.fetched_at, rating.expires_at,
                   COALESCE((
                       SELECT lifecycle.status
                       FROM metadata_claim_lifecycle_events lifecycle
                       WHERE lifecycle.claim_id = rating.claim_id
                       ORDER BY lifecycle.sequence DESC LIMIT 1
                   ), rating.initial_status)
            FROM metadata_rating_claims rating
            WHERE rating.workspace_id = ?1 AND rating.record_id = ?2
            ORDER BY rating.fetched_at DESC, rating.claim_id DESC
            LIMIT ?3
            "#,
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map(
            params![
                workspace_id.to_string(),
                record_id.to_string(),
                MAX_EFFECTIVE_FIELD_CLAIMS
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, Option<String>>(12)?,
                    row.get::<_, String>(13)?,
                ))
            },
        ),
        capability,
        correlation_id,
    )?;
    let mut claims = Vec::new();
    for row in rows {
        let (
            claim_id,
            value_millis,
            scale_minimum,
            scale_maximum,
            provider_id,
            source,
            source_record_id,
            locale,
            region,
            source_version,
            evidence_digest,
            fetched_at,
            expires_at,
            status,
        ) = map_sql(row, capability, correlation_id)?;
        let provenance = FieldClaimProvenance::try_new(
            MetadataProviderId::try_new(provider_id).map_err(|_| {
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            })?,
            NamespaceKey::try_new(source).map_err(|_| {
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            })?,
            source_record_id,
            locale
                .map(MetadataLocale::try_new)
                .transpose()
                .map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
            region
                .map(MetadataRegion::try_new)
                .transpose()
                .map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
            source_version,
            evidence_digest.parse::<Sha256Digest>().map_err(|_| {
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            })?,
        )
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        let claim = RatingClaim::try_new(
            claim_id.parse::<MetadataClaimId>().map_err(|_| {
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            })?,
            record_id,
            u32::try_from(value_millis).map_err(|_| {
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            })?,
            RatingScale::try_new(
                u32::try_from(scale_minimum).map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
                u32::try_from(scale_maximum).map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
            )
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
            provenance,
            ReceivedAt::from_application_clock(parse_timestamp(
                &fetched_at,
                capability,
                correlation_id,
            )?),
            expires_at
                .map(|value| parse_timestamp(&value, capability, correlation_id))
                .transpose()?,
            parse_claim_status(&status).ok_or_else(|| {
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            })?,
        )
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        claims.push(claim);
    }
    Ok(claims)
}

#[allow(dead_code)]
pub(crate) fn write_metadata_cache_entry(
    connection: &Connection,
    workspace_id: WorkspaceId,
    entry: &MetadataCacheEntry,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    let key = entry.key();
    let cache_key = cache_storage_key(key)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    map_sql(
        connection.execute_batch("SAVEPOINT metadata_cache_write"),
        capability,
        correlation_id,
    )?;
    let result = (|| {
        let (invalidation_reason_value, invalidated_at) = entry
            .invalidation()
            .map(|invalidation| {
                (
                    Some(invalidation_reason(invalidation.reason())),
                    Some(timestamp(invalidation.invalidated_at())),
                )
            })
            .unwrap_or((None, None));
        let changed = map_sql(
            connection.execute(
                r#"
                INSERT INTO metadata_cache_entries(
                    cache_key, workspace_id, provider_id, settings_fingerprint,
                    configuration_digest, credential_reference_version,
                    record_id, route, grain, identifier_namespace,
                    identifier_value, locale, region, field_group,
                    schema_version, purpose, terms_revision, classification,
                    invalidation_reason, invalidated_at, fresh_until,
                    stale_while_refreshing_until, stale_on_error_until,
                    created_at, updated_at
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                    ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23,
                    ?24, ?25
                )
                ON CONFLICT(cache_key) DO UPDATE SET
                    invalidation_reason = excluded.invalidation_reason,
                    invalidated_at = excluded.invalidated_at,
                    fresh_until = excluded.fresh_until,
                    stale_while_refreshing_until = excluded.stale_while_refreshing_until,
                    stale_on_error_until = excluded.stale_on_error_until,
                    created_at = excluded.created_at,
                    updated_at = excluded.updated_at
                WHERE metadata_cache_entries.workspace_id = excluded.workspace_id
                  AND metadata_cache_entries.provider_id = excluded.provider_id
                  AND metadata_cache_entries.settings_fingerprint = excluded.settings_fingerprint
                  AND metadata_cache_entries.configuration_digest = excluded.configuration_digest
                  AND metadata_cache_entries.credential_reference_version
                        IS excluded.credential_reference_version
                  AND metadata_cache_entries.record_id = excluded.record_id
                  AND metadata_cache_entries.route = excluded.route
                  AND metadata_cache_entries.grain = excluded.grain
                  AND metadata_cache_entries.identifier_namespace = excluded.identifier_namespace
                  AND metadata_cache_entries.identifier_value = excluded.identifier_value
                  AND metadata_cache_entries.locale IS excluded.locale
                  AND metadata_cache_entries.region IS excluded.region
                  AND metadata_cache_entries.field_group = excluded.field_group
                  AND metadata_cache_entries.schema_version = excluded.schema_version
                  AND metadata_cache_entries.purpose = excluded.purpose
                  AND metadata_cache_entries.terms_revision = excluded.terms_revision
                  AND metadata_cache_entries.classification = excluded.classification
                "#,
                params![
                    cache_key,
                    workspace_id.to_string(),
                    key.provider_id().as_str(),
                    key.settings_fingerprint().to_string(),
                    key.configuration_digest().to_string(),
                    key.credential_reference_version().map(|value| value as i64),
                    key.record_id().to_string(),
                    key.resolved_provider_route(),
                    key.grain().as_str(),
                    key.source_namespace().as_str(),
                    key.source_identifier(),
                    key.locale().map(|value| value.as_str()),
                    key.region().map(|value| value.as_str()),
                    field_group(key.field_group()),
                    i64::from(key.schema_version()),
                    cache_purpose(key.purpose()),
                    key.terms_revision(),
                    data_classification(key.classification()),
                    invalidation_reason_value,
                    invalidated_at,
                    timestamp(entry.fresh_until()),
                    timestamp(entry.stale_while_refreshing_until()),
                    timestamp(entry.stale_on_error_until()),
                    timestamp(entry.created_at()),
                    timestamp(now())
                ],
            ),
            capability,
            correlation_id,
        )?;
        if changed != 1 {
            return Err(immutable_claim_conflict(capability, correlation_id));
        }
        map_sql(
            connection.execute(
                "DELETE FROM metadata_cache_claims WHERE cache_key = ?1 AND workspace_id = ?2",
                params![cache_key, workspace_id.to_string()],
            ),
            capability,
            correlation_id,
        )?;
        for (ordinal, claim_id) in entry.claim_ids().iter().enumerate() {
            map_sql(
                connection.execute(
                    r#"
                    INSERT INTO metadata_cache_claims(
                        cache_key, workspace_id, ordinal, claim_id
                    ) VALUES (?1, ?2, ?3, ?4)
                    "#,
                    params![
                        cache_key,
                        workspace_id.to_string(),
                        ordinal as i64,
                        claim_id.to_string()
                    ],
                ),
                capability,
                correlation_id,
            )?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => map_sql(
            connection.execute_batch("RELEASE metadata_cache_write"),
            capability,
            correlation_id,
        ),
        Err(error) => {
            let _ = connection
                .execute_batch("ROLLBACK TO metadata_cache_write; RELEASE metadata_cache_write");
            Err(error)
        }
    }
}

#[allow(dead_code)]
pub(crate) fn load_metadata_cache_entry(
    connection: &Connection,
    workspace_id: WorkspaceId,
    key: &MetadataCacheKey,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<Option<MetadataCacheEntry>> {
    let cache_key = cache_storage_key(key)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let row = map_sql(
        connection
            .query_row(
                r#"
                SELECT provider_id, credential_reference_version, record_id,
                       route, grain, identifier_namespace, identifier_value,
                       locale, region, field_group, settings_fingerprint,
                       configuration_digest, schema_version, purpose,
                       terms_revision, classification, created_at, fresh_until,
                       stale_while_refreshing_until, stale_on_error_until,
                       invalidation_reason, invalidated_at
                FROM metadata_cache_entries
                WHERE cache_key = ?1 AND workspace_id = ?2
                "#,
                params![cache_key, workspace_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<i64>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?,
                        row.get::<_, String>(11)?,
                        row.get::<_, i64>(12)?,
                        row.get::<_, String>(13)?,
                        row.get::<_, String>(14)?,
                        row.get::<_, String>(15)?,
                        row.get::<_, String>(16)?,
                        row.get::<_, String>(17)?,
                        row.get::<_, String>(18)?,
                        row.get::<_, String>(19)?,
                        row.get::<_, Option<String>>(20)?,
                        row.get::<_, Option<String>>(21)?,
                    ))
                },
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    let Some((
        provider_id,
        credential_reference_version,
        stored_record_id,
        route,
        grain,
        source_namespace,
        source_identifier,
        locale,
        region,
        stored_field_group,
        settings_fingerprint,
        configuration_digest,
        schema_version,
        purpose,
        terms_revision,
        classification,
        created_at,
        fresh_until,
        stale_while_refreshing_until,
        stale_on_error_until,
        stored_invalidation_reason,
        invalidated_at,
    )) = row
    else {
        return Ok(None);
    };
    let stored_key = MetadataCacheKey::try_new(
        MetadataProviderId::try_new(provider_id)
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        credential_reference_version
            .map(u64::try_from)
            .transpose()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        stored_record_id
            .parse::<RecordId>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        route,
        grain
            .parse()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        NamespaceKey::try_new(source_namespace)
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        source_identifier,
        locale
            .map(MetadataLocale::try_new)
            .transpose()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        region
            .map(MetadataRegion::try_new)
            .transpose()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        parse_field_group(&stored_field_group)
            .ok_or_else(|| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        settings_fingerprint
            .parse::<Sha256Digest>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        configuration_digest
            .parse::<Sha256Digest>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        u32::try_from(schema_version)
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        parse_cache_purpose(&purpose)
            .ok_or_else(|| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        terms_revision,
        parse_data_classification(&classification)
            .ok_or_else(|| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
    )
    .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    if &stored_key != key {
        return Err(Box::new(FastiProblem::integrity_failed(
            capability,
            correlation_id,
        )));
    }
    let mut claim_statement = map_sql(
        connection.prepare(
            r#"
            SELECT claim_id FROM metadata_cache_claims
            WHERE cache_key = ?1 AND workspace_id = ?2
            ORDER BY ordinal
            LIMIT 256
            "#,
        ),
        capability,
        correlation_id,
    )?;
    let claim_rows = map_sql(
        claim_statement.query_map(params![cache_key, workspace_id.to_string()], |row| {
            row.get::<_, String>(0)
        }),
        capability,
        correlation_id,
    )?;
    let mut claim_ids = Vec::new();
    for claim_id in claim_rows {
        claim_ids.push(
            map_sql(claim_id, capability, correlation_id)?
                .parse::<MetadataClaimId>()
                .map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
        );
    }
    let mut entry = MetadataCacheEntry::try_new(
        stored_key,
        claim_ids,
        ReceivedAt::from_application_clock(parse_timestamp(
            &created_at,
            capability,
            correlation_id,
        )?),
        parse_timestamp(&fresh_until, capability, correlation_id)?,
        parse_timestamp(&stale_while_refreshing_until, capability, correlation_id)?,
        parse_timestamp(&stale_on_error_until, capability, correlation_id)?,
    )
    .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    match (stored_invalidation_reason, invalidated_at) {
        (None, None) => {}
        (Some(reason), Some(invalidated_at)) => {
            entry = entry
                .invalidated(
                    parse_invalidation_reason(&reason).ok_or_else(|| {
                        Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                    })?,
                    ReceivedAt::from_application_clock(parse_timestamp(
                        &invalidated_at,
                        capability,
                        correlation_id,
                    )?),
                )
                .map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?;
        }
        _ => {
            return Err(Box::new(FastiProblem::integrity_failed(
                capability,
                correlation_id,
            )))
        }
    }
    Ok(Some(entry))
}

#[allow(dead_code)]
pub(crate) fn clear_metadata_cache_partition(
    connection: &Connection,
    workspace_id: WorkspaceId,
    provider_id: Option<&MetadataProviderId>,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<usize> {
    map_sql(
        match provider_id {
            Some(provider_id) => connection.execute(
                "DELETE FROM metadata_cache_entries WHERE workspace_id = ?1 AND provider_id = ?2",
                params![workspace_id.to_string(), provider_id.as_str()],
            ),
            None => connection.execute(
                "DELETE FROM metadata_cache_entries WHERE workspace_id = ?1",
                [workspace_id.to_string()],
            ),
        },
        capability,
        correlation_id,
    )
}

fn invalid_provider_metadata(
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Box<FastiProblem> {
    Box::new(FastiProblem::from_code(
        ProblemCode::ValidationFailed,
        capability,
        correlation_id,
    ))
}

fn immutable_claim_conflict(
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Box<FastiProblem> {
    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
}

fn write_provider_fields(
    transaction: &Transaction<'_>,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    identifier: &fasti_domain::ExternalIdentifierClaim,
    fields: &[ProviderMetadataField],
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    if fields.is_empty() || fields.len() > MAX_PROVIDER_METADATA_FIELDS {
        return Err(invalid_provider_metadata(capability, correlation_id));
    }
    let mut keys = BTreeSet::new();
    for field in fields {
        if field.claim().source().as_str() != identifier.namespace()
            || !keys.insert(field.field_key().as_str())
        {
            return Err(invalid_provider_metadata(capability, correlation_id));
        }
        write_field_claim(
            transaction,
            workspace_id,
            record_id,
            field.field_key(),
            field.claim(),
            capability,
            correlation_id,
        )?;
    }
    Ok(())
}

impl ProviderMetadataPort for SqliteKernel {
    fn create_provider_record(
        &self,
        command: CreateProviderRecordCommand,
    ) -> ApplicationResult<CreateProviderRecordOutcome> {
        let correlation_id = command.correlation_id();
        let capability = CapabilityKey::AttachIdentifier;
        if command.grain() != command.identifier().grain() {
            return Err(invalid_provider_metadata(capability, correlation_id));
        }
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        let workspace_id = command.access().workspace_id();
        let existing = matching_record_ids(
            &transaction,
            workspace_id,
            std::slice::from_ref(command.identifier()),
            capability,
            correlation_id,
        )?;
        let record_id = if let Some(record_id) = existing.first() {
            *record_id
        } else {
            insert_record(
                &transaction,
                workspace_id,
                command.grain(),
                capability,
                correlation_id,
            )?
        };
        attach_identifier_tx(
            &transaction,
            workspace_id,
            record_id,
            command.identifier(),
            capability,
            correlation_id,
        )?;
        write_provider_fields(
            &transaction,
            workspace_id,
            record_id,
            command.identifier(),
            command.fields(),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(CreateProviderRecordOutcome::new(record_id, command.grain()))
    }

    fn apply_provider_metadata(
        &self,
        command: ApplyProviderMetadataCommand,
    ) -> ApplicationResult<()> {
        let correlation_id = command.correlation_id();
        let capability = CapabilityKey::AttachIdentifier;
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        let workspace_id = command.access().workspace_id();
        attach_identifier_tx(
            &transaction,
            workspace_id,
            command.record_id(),
            command.identifier(),
            capability,
            correlation_id,
        )?;
        write_provider_fields(
            &transaction,
            workspace_id,
            command.record_id(),
            command.identifier(),
            command.fields(),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(())
    }
}

#[allow(dead_code)]
pub(crate) fn write_field_override(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    field_key: &FieldKey,
    override_: &FieldOverride,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    map_sql(
        connection.execute(
            r#"
            INSERT INTO metadata_field_overrides(
                workspace_id, record_id, field_key, value, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(record_id, field_key) DO UPDATE SET
                workspace_id = excluded.workspace_id,
                value = excluded.value,
                created_at = excluded.created_at
            "#,
            params![
                workspace_id.to_string(),
                record_id.to_string(),
                field_key.as_str(),
                override_.value(),
                timestamp(override_.created_at())
            ],
        ),
        capability,
        correlation_id,
    )?;
    Ok(())
}

pub(crate) fn load_field_claims(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    field_key: &FieldKey,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<Vec<FieldClaim>> {
    let mut statement = map_sql(
        connection.prepare(
            r#"
            SELECT claim.record_id, claim.field_key,
                   provenance.claim_id, claim.source, claim.value, claim.locale,
                   provenance.provider_id, provenance.source_record_id,
                   provenance.region, provenance.source_version,
                   provenance.evidence_digest, provenance.provenance_state,
                   claim.fetched_at, claim.expires_at,
                   COALESCE((
                       SELECT lifecycle.status
                       FROM metadata_claim_lifecycle_events lifecycle
                       WHERE lifecycle.claim_id = provenance.claim_id
                       ORDER BY lifecycle.sequence DESC
                       LIMIT 1
                   ), provenance.initial_status)
            FROM metadata_field_claims claim
            JOIN metadata_claim_provenance provenance
              ON provenance.record_id = claim.record_id
             AND provenance.field_key = claim.field_key
             AND provenance.source = claim.source
             AND provenance.fetched_at = claim.fetched_at
            WHERE claim.workspace_id = ?1
              AND claim.record_id = ?2
              AND claim.field_key = ?3
            ORDER BY claim.fetched_at DESC, claim.source DESC
            LIMIT ?4
            "#,
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map(
            params![
                workspace_id.to_string(),
                record_id.to_string(),
                field_key.as_str(),
                MAX_EFFECTIVE_FIELD_CLAIMS
            ],
            PersistedFieldClaimRow::read,
        ),
        capability,
        correlation_id,
    )?;
    let mut claims = Vec::new();
    for row in rows {
        let ((loaded_record_id, loaded_field_key), claim) =
            map_sql(row, capability, correlation_id)?.decode(capability, correlation_id)?;
        if loaded_record_id != record_id || loaded_field_key != *field_key {
            return Err(Box::new(FastiProblem::integrity_failed(
                capability,
                correlation_id,
            )));
        }
        claims.push(claim);
    }
    Ok(claims)
}

struct PersistedFieldClaimRow {
    record_id: String,
    field_key: String,
    claim_id: String,
    source: String,
    value: String,
    locale: Option<String>,
    provider_id: Option<String>,
    source_record_id: Option<String>,
    region: Option<String>,
    source_version: Option<String>,
    evidence_digest: Option<String>,
    provenance_state: String,
    fetched_at: String,
    expires_at: Option<String>,
    status: String,
}

impl PersistedFieldClaimRow {
    fn read(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            record_id: row.get(0)?,
            field_key: row.get(1)?,
            claim_id: row.get(2)?,
            source: row.get(3)?,
            value: row.get(4)?,
            locale: row.get(5)?,
            provider_id: row.get(6)?,
            source_record_id: row.get(7)?,
            region: row.get(8)?,
            source_version: row.get(9)?,
            evidence_digest: row.get(10)?,
            provenance_state: row.get(11)?,
            fetched_at: row.get(12)?,
            expires_at: row.get(13)?,
            status: row.get(14)?,
        })
    }

    fn decode(
        self,
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<((RecordId, FieldKey), FieldClaim)> {
        let integrity = || Box::new(FastiProblem::integrity_failed(capability, correlation_id));
        let record_id = self
            .record_id
            .parse::<RecordId>()
            .map_err(|_| integrity())?;
        let field_key = FieldKey::try_new(self.field_key).map_err(|_| integrity())?;
        let claim_id = self
            .claim_id
            .parse::<MetadataClaimId>()
            .map_err(|_| integrity())?;
        let source = NamespaceKey::try_new(self.source).map_err(|_| integrity())?;
        let locale = self
            .locale
            .map(MetadataLocale::try_new)
            .transpose()
            .map_err(|_| integrity())?;
        let provenance = match self.provenance_state.as_str() {
            "complete" => FieldClaimProvenance::try_new(
                MetadataProviderId::try_new(self.provider_id.ok_or_else(integrity)?)
                    .map_err(|_| integrity())?,
                source,
                self.source_record_id.ok_or_else(integrity)?,
                locale,
                self.region
                    .map(MetadataRegion::try_new)
                    .transpose()
                    .map_err(|_| integrity())?,
                self.source_version,
                self.evidence_digest
                    .ok_or_else(integrity)?
                    .parse::<Sha256Digest>()
                    .map_err(|_| integrity())?,
            )
            .map_err(|_| integrity())?,
            "legacy_incomplete" => FieldClaimProvenance::legacy(source, locale),
            _ => return Err(integrity()),
        };
        let fetched_at = ReceivedAt::from_application_clock(parse_timestamp(
            &self.fetched_at,
            capability,
            correlation_id,
        )?);
        let expires_at = self
            .expires_at
            .map(|value| parse_timestamp(&value, capability, correlation_id))
            .transpose()?;
        let status = parse_claim_status(&self.status).ok_or_else(integrity)?;
        let claim = FieldClaim::try_from_persisted(
            claim_id,
            Some(record_id),
            Some(field_key.clone()),
            self.value,
            provenance,
            fetched_at,
            expires_at,
            status,
        )
        .map_err(|_: FieldClaimError| integrity())?;
        Ok(((record_id, field_key), claim))
    }
}

pub(crate) struct RecordListMetadata {
    policy: MetadataProjectionPolicy,
    claims: HashMap<(RecordId, FieldKey), Vec<FieldClaim>>,
    overrides: HashMap<(RecordId, FieldKey), ProfileFieldOverride>,
    resolved_at: chrono::DateTime<chrono::Utc>,
}

impl RecordListMetadata {
    pub(crate) fn resolve(
        &self,
        record_id: RecordId,
        field_key: &FieldKey,
        capability: CapabilityKey,
        correlation_id: RequestCorrelationId,
    ) -> ApplicationResult<fasti_domain::ResolvedField> {
        let key = (record_id, field_key.clone());
        resolve_profile_field(
            self.overrides.get(&key),
            self.claims.get(&key).map(Vec::as_slice).unwrap_or_default(),
            &[],
            &self.policy,
            self.resolved_at,
        )
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))
    }
}

pub(crate) fn load_record_list_metadata(
    connection: &Connection,
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    page_limit: i64,
    field_keys: &[FieldKey; 5],
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<RecordListMetadata> {
    let policy = load_projection_policy(
        connection,
        workspace_id,
        profile_id,
        capability,
        correlation_id,
    )?;
    let mut statement = map_sql(
        connection.prepare(
            r#"
            WITH page_records AS (
                SELECT record_id FROM records
                WHERE workspace_id = ?1 AND status = 'active'
                ORDER BY record_id
                LIMIT ?2
            ), ranked_claims AS (
                SELECT claim.record_id, claim.field_key,
                       provenance.claim_id, claim.source, claim.value, claim.locale,
                       provenance.provider_id, provenance.source_record_id,
                       provenance.region, provenance.source_version,
                       provenance.evidence_digest, provenance.provenance_state,
                       claim.fetched_at, claim.expires_at,
                       COALESCE((
                           SELECT lifecycle.status
                           FROM metadata_claim_lifecycle_events lifecycle
                           WHERE lifecycle.claim_id = provenance.claim_id
                           ORDER BY lifecycle.sequence DESC
                           LIMIT 1
                       ), provenance.initial_status) AS status,
                       ROW_NUMBER() OVER (
                           PARTITION BY claim.record_id, claim.field_key
                           ORDER BY claim.fetched_at DESC, claim.source DESC
                       ) AS claim_rank
                FROM metadata_field_claims claim
                JOIN page_records page ON page.record_id = claim.record_id
                JOIN metadata_claim_provenance provenance
                  ON provenance.workspace_id = claim.workspace_id
                 AND provenance.record_id = claim.record_id
                 AND provenance.field_key = claim.field_key
                 AND provenance.source = claim.source
                 AND provenance.fetched_at = claim.fetched_at
                WHERE claim.workspace_id = ?1
                  AND claim.field_key IN (?3, ?4, ?5, ?6, ?7)
            )
            SELECT record_id, field_key, claim_id, source, value, locale,
                   provider_id, source_record_id, region, source_version,
                   evidence_digest, provenance_state, fetched_at, expires_at, status
            FROM ranked_claims
            WHERE claim_rank <= ?8
            ORDER BY record_id, field_key, fetched_at DESC, source DESC
            "#,
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map(
            params![
                workspace_id.to_string(),
                page_limit,
                field_keys[0].as_str(),
                field_keys[1].as_str(),
                field_keys[2].as_str(),
                field_keys[3].as_str(),
                field_keys[4].as_str(),
                MAX_EFFECTIVE_FIELD_CLAIMS,
            ],
            PersistedFieldClaimRow::read,
        ),
        capability,
        correlation_id,
    )?;
    let mut claims: HashMap<(RecordId, FieldKey), Vec<FieldClaim>> = HashMap::new();
    for row in rows {
        let (key, claim) =
            map_sql(row, capability, correlation_id)?.decode(capability, correlation_id)?;
        claims.entry(key).or_default().push(claim);
    }
    drop(statement);

    let mut statement = map_sql(
        connection.prepare(
            r#"
            WITH page_records AS (
                SELECT record_id FROM records
                WHERE workspace_id = ?1 AND status = 'active'
                ORDER BY record_id
                LIMIT ?2
            )
            SELECT override.record_id, override.field_key,
                   override.value, override.created_at
            FROM metadata_profile_field_overrides override
            JOIN page_records page ON page.record_id = override.record_id
            WHERE override.workspace_id = ?1
              AND override.profile_id = ?3
              AND override.field_key IN (?4, ?5, ?6, ?7, ?8)
            ORDER BY override.record_id, override.field_key
            "#,
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map(
            params![
                workspace_id.to_string(),
                page_limit,
                profile_id.to_string(),
                field_keys[0].as_str(),
                field_keys[1].as_str(),
                field_keys[2].as_str(),
                field_keys[3].as_str(),
                field_keys[4].as_str(),
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        ),
        capability,
        correlation_id,
    )?;
    let mut overrides = HashMap::new();
    for row in rows {
        let (record_id, field_key, value, created_at) = map_sql(row, capability, correlation_id)?;
        let record_id = record_id
            .parse::<RecordId>()
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        let field_key = FieldKey::try_new(field_key)
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        let override_ = ProfileFieldOverride::try_new(
            profile_id,
            record_id,
            field_key.clone(),
            value,
            ReceivedAt::from_application_clock(parse_timestamp(
                &created_at,
                capability,
                correlation_id,
            )?),
        )
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        if overrides
            .insert((record_id, field_key), override_)
            .is_some()
        {
            return Err(Box::new(FastiProblem::integrity_failed(
                capability,
                correlation_id,
            )));
        }
    }
    Ok(RecordListMetadata {
        policy,
        claims,
        overrides,
        resolved_at: now(),
    })
}

#[allow(dead_code)]
pub(crate) fn load_field_override(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    field_key: &FieldKey,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<Option<FieldOverride>> {
    let row = map_sql(
        connection
            .query_row(
                r#"
                SELECT value, created_at FROM metadata_field_overrides
                WHERE workspace_id = ?1 AND record_id = ?2 AND field_key = ?3
                "#,
                params![
                    workspace_id.to_string(),
                    record_id.to_string(),
                    field_key.as_str()
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    let Some((value, created_at)) = row else {
        return Ok(None);
    };
    let created_at = ReceivedAt::from_application_clock(parse_timestamp(
        &created_at,
        capability,
        correlation_id,
    )?);
    let override_ = FieldOverride::try_new(value, created_at)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    Ok(Some(override_))
}

#[allow(dead_code)]
pub(crate) fn write_profile_field_override(
    connection: &Connection,
    workspace_id: WorkspaceId,
    override_: &ProfileFieldOverride,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    map_sql(
        connection.execute(
            r#"
            INSERT INTO metadata_profile_field_overrides(
                workspace_id, profile_id, record_id, field_key, value,
                created_at, updated_at, origin
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, 'user')
            ON CONFLICT(workspace_id, profile_id, record_id, field_key) DO UPDATE SET
                value = excluded.value,
                created_at = excluded.created_at,
                updated_at = excluded.updated_at,
                origin = 'user'
            "#,
            params![
                workspace_id.to_string(),
                override_.profile_id().to_string(),
                override_.record_id().to_string(),
                override_.field_key().as_str(),
                override_.value(),
                timestamp(override_.created_at())
            ],
        ),
        capability,
        correlation_id,
    )?;
    Ok(())
}

pub(crate) fn load_profile_field_override(
    connection: &Connection,
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    record_id: RecordId,
    field_key: &FieldKey,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<Option<ProfileFieldOverride>> {
    let row = map_sql(
        connection
            .query_row(
                r#"
                SELECT value, created_at
                FROM metadata_profile_field_overrides
                WHERE workspace_id = ?1 AND profile_id = ?2
                  AND record_id = ?3 AND field_key = ?4
                "#,
                params![
                    workspace_id.to_string(),
                    profile_id.to_string(),
                    record_id.to_string(),
                    field_key.as_str()
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    row.map(|(value, created_at)| {
        ProfileFieldOverride::try_new(
            profile_id,
            record_id,
            field_key.clone(),
            value,
            ReceivedAt::from_application_clock(parse_timestamp(
                &created_at,
                capability,
                correlation_id,
            )?),
        )
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))
    })
    .transpose()
}

#[allow(dead_code)]
pub(crate) fn write_projection_policy(
    connection: &Connection,
    workspace_id: WorkspaceId,
    policy: &MetadataProjectionPolicy,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    map_sql(
        connection.execute(
            r#"
            INSERT INTO metadata_projection_policies(
                workspace_id, profile_id, preferred_provider_id,
                preferred_locale, original_locale, allow_english_fallback,
                last_known_good_policy, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(workspace_id, profile_id) DO UPDATE SET
                preferred_provider_id = excluded.preferred_provider_id,
                preferred_locale = excluded.preferred_locale,
                original_locale = excluded.original_locale,
                allow_english_fallback = excluded.allow_english_fallback,
                last_known_good_policy = excluded.last_known_good_policy,
                updated_at = excluded.updated_at
            "#,
            params![
                workspace_id.to_string(),
                policy.profile_id().to_string(),
                policy.preferred_provider_id().map(|value| value.as_str()),
                policy.preferred_locale().map(|value| value.as_str()),
                policy.original_locale().map(|value| value.as_str()),
                i64::from(policy.allow_english_fallback()),
                match policy.last_known_good() {
                    LastKnownGoodPolicy::Allow => "allow",
                    LastKnownGoodPolicy::Deny => "deny",
                },
                timestamp(now())
            ],
        ),
        capability,
        correlation_id,
    )?;
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn write_enrichment_policy(
    connection: &Connection,
    workspace_id: WorkspaceId,
    policy: &EnrichmentPolicy,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    let projection = policy.projection_policy();
    let enabled_field_groups = encode_field_groups(policy.enabled_field_groups())
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    map_sql(
        connection.execute(
            r#"
            INSERT INTO metadata_projection_policies(
                workspace_id, profile_id, preferred_provider_id,
                preferred_locale, original_locale, region, enabled_field_groups,
                allow_english_fallback, last_known_good_policy, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(workspace_id, profile_id) DO UPDATE SET
                preferred_provider_id = excluded.preferred_provider_id,
                preferred_locale = excluded.preferred_locale,
                original_locale = excluded.original_locale,
                region = excluded.region,
                enabled_field_groups = excluded.enabled_field_groups,
                allow_english_fallback = excluded.allow_english_fallback,
                last_known_good_policy = excluded.last_known_good_policy,
                updated_at = excluded.updated_at
            "#,
            params![
                workspace_id.to_string(),
                policy.profile_id().to_string(),
                projection
                    .preferred_provider_id()
                    .map(|value| value.as_str()),
                projection.preferred_locale().map(|value| value.as_str()),
                projection.original_locale().map(|value| value.as_str()),
                policy.region().map(|value| value.as_str()),
                enabled_field_groups,
                i64::from(projection.allow_english_fallback()),
                match projection.last_known_good() {
                    LastKnownGoodPolicy::Allow => "allow",
                    LastKnownGoodPolicy::Deny => "deny",
                },
                timestamp(now())
            ],
        ),
        capability,
        correlation_id,
    )?;
    Ok(())
}

pub(crate) fn load_projection_policy(
    connection: &Connection,
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<MetadataProjectionPolicy> {
    let row = map_sql(
        connection
            .query_row(
                r#"
                SELECT preferred_provider_id, preferred_locale, original_locale,
                       allow_english_fallback, last_known_good_policy
                FROM metadata_projection_policies
                WHERE workspace_id = ?1 AND profile_id = ?2
                "#,
                params![workspace_id.to_string(), profile_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    let Some((provider_id, preferred_locale, original_locale, english_fallback, last_known_good)) =
        row
    else {
        return Ok(MetadataProjectionPolicy::default_for_profile(profile_id));
    };
    let provider_id = provider_id
        .map(MetadataProviderId::try_new)
        .transpose()
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let preferred_locale = preferred_locale
        .map(MetadataLocale::try_new)
        .transpose()
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let original_locale = original_locale
        .map(MetadataLocale::try_new)
        .transpose()
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let last_known_good = match last_known_good.as_str() {
        "allow" => LastKnownGoodPolicy::Allow,
        "deny" => LastKnownGoodPolicy::Deny,
        _ => {
            return Err(Box::new(FastiProblem::integrity_failed(
                capability,
                correlation_id,
            )))
        }
    };
    Ok(MetadataProjectionPolicy::new(
        profile_id,
        provider_id,
        preferred_locale,
        original_locale,
        english_fallback == 1,
        last_known_good,
    ))
}

#[allow(dead_code)]
pub(crate) fn load_enrichment_policy(
    connection: &Connection,
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<EnrichmentPolicy> {
    let row = map_sql(
        connection
            .query_row(
                r#"
                SELECT region, enabled_field_groups
                FROM metadata_projection_policies
                WHERE workspace_id = ?1 AND profile_id = ?2
                "#,
                params![workspace_id.to_string(), profile_id.to_string()],
                |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?)),
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    let projection = load_projection_policy(
        connection,
        workspace_id,
        profile_id,
        capability,
        correlation_id,
    )?;
    let Some((region, enabled_field_groups)) = row else {
        return Ok(EnrichmentPolicy::new(projection, None, Vec::new()));
    };
    let region = region
        .map(MetadataRegion::try_new)
        .transpose()
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let enabled_field_groups = decode_field_groups(&enabled_field_groups)
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    Ok(EnrichmentPolicy::new(
        projection,
        region,
        enabled_field_groups,
    ))
}

#[allow(dead_code)]
pub(crate) fn write_metadata_projection(
    connection: &Connection,
    workspace_id: WorkspaceId,
    projection: &MetadataProjection,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    let resolved = projection.resolved_field();
    map_sql(
        connection.execute(
            r#"
            INSERT INTO metadata_projections(
                workspace_id, profile_id, record_id, field_key,
                resolution_tier, value, claim_id, is_stale, projected_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(workspace_id, profile_id, record_id, field_key) DO UPDATE SET
                resolution_tier = excluded.resolution_tier,
                value = excluded.value,
                claim_id = excluded.claim_id,
                is_stale = excluded.is_stale,
                projected_at = excluded.projected_at
            "#,
            params![
                workspace_id.to_string(),
                projection.profile_id().to_string(),
                projection.record_id().to_string(),
                projection.field_key().as_str(),
                resolution_tier(resolved.tier()),
                resolved.value(),
                resolved
                    .provenance()
                    .map(|provenance| provenance.claim_id().to_string()),
                i64::from(resolved.is_stale()),
                timestamp(projection.projected_at())
            ],
        ),
        capability,
        correlation_id,
    )?;
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn write_metadata_attribution(
    connection: &Connection,
    workspace_id: WorkspaceId,
    attribution: &MetadataAttribution,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    map_sql(
        connection.execute(
            r#"
            INSERT INTO metadata_attributions(
                workspace_id, provider_id, attribution_text,
                documentation_url, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(workspace_id, provider_id) DO UPDATE SET
                attribution_text = excluded.attribution_text,
                documentation_url = excluded.documentation_url,
                updated_at = excluded.updated_at
            "#,
            params![
                workspace_id.to_string(),
                attribution.provider_id().as_str(),
                attribution.text(),
                attribution.documentation_url(),
                timestamp(now())
            ],
        ),
        capability,
        correlation_id,
    )?;
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn load_metadata_attribution(
    connection: &Connection,
    workspace_id: WorkspaceId,
    provider_id: &MetadataProviderId,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<Option<MetadataAttribution>> {
    let row = map_sql(
        connection
            .query_row(
                r#"
                SELECT attribution_text, documentation_url
                FROM metadata_attributions
                WHERE workspace_id = ?1 AND provider_id = ?2
                "#,
                params![workspace_id.to_string(), provider_id.as_str()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    row.map(|(text, documentation_url)| {
        MetadataAttribution::try_new(provider_id.clone(), text, documentation_url)
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))
    })
    .transpose()
}

fn load_record_attributions(
    connection: &Connection,
    workspace_id: WorkspaceId,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<Vec<MetadataAttribution>> {
    let mut statement = map_sql(
        connection.prepare(
            r#"
            SELECT provider_id, attribution_text, documentation_url
            FROM metadata_attributions
            WHERE workspace_id = ?1
            ORDER BY provider_id
            LIMIT 64
            "#,
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map([workspace_id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        }),
        capability,
        correlation_id,
    )?;
    let mut attributions = Vec::new();
    for row in rows {
        let (provider_id, text, documentation_url) = map_sql(row, capability, correlation_id)?;
        attributions.push(
            MetadataAttribution::try_new(
                MetadataProviderId::try_new(provider_id).map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
                text,
                documentation_url,
            )
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        );
    }
    Ok(attributions)
}

fn load_record_cache_keys(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    settings_fingerprint: &Sha256Digest,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<Vec<MetadataCacheKey>> {
    let mut statement = map_sql(
        connection.prepare(
            r#"
            SELECT provider_id, credential_reference_version, route, grain,
                   identifier_namespace, identifier_value, locale, region,
                   field_group, settings_fingerprint, configuration_digest,
                   schema_version, purpose, terms_revision, classification
            FROM metadata_cache_entries
            WHERE workspace_id = ?1 AND record_id = ?2 AND settings_fingerprint = ?3
            ORDER BY provider_id, cache_key
            LIMIT 256
            "#,
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map(
            params![
                workspace_id.to_string(),
                record_id.to_string(),
                settings_fingerprint.to_string()
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, i64>(11)?,
                    row.get::<_, String>(12)?,
                    row.get::<_, String>(13)?,
                    row.get::<_, String>(14)?,
                ))
            },
        ),
        capability,
        correlation_id,
    )?;
    let mut keys = Vec::new();
    for row in rows {
        let (
            provider_id,
            credential_reference_version,
            route,
            grain,
            source_namespace,
            source_identifier,
            locale,
            region,
            field_group_value,
            settings_fingerprint,
            configuration_digest,
            schema_version,
            purpose,
            terms_revision,
            classification,
        ) = map_sql(row, capability, correlation_id)?;
        keys.push(
            MetadataCacheKey::try_new(
                MetadataProviderId::try_new(provider_id).map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
                credential_reference_version
                    .map(u64::try_from)
                    .transpose()
                    .map_err(|_| {
                        Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                    })?,
                record_id,
                route,
                grain.parse().map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
                NamespaceKey::try_new(source_namespace).map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
                source_identifier,
                locale
                    .map(MetadataLocale::try_new)
                    .transpose()
                    .map_err(|_| {
                        Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                    })?,
                region
                    .map(MetadataRegion::try_new)
                    .transpose()
                    .map_err(|_| {
                        Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                    })?,
                parse_field_group(&field_group_value).ok_or_else(|| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
                settings_fingerprint.parse().map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
                configuration_digest.parse().map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
                u32::try_from(schema_version).map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
                parse_cache_purpose(&purpose).ok_or_else(|| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
                terms_revision,
                parse_data_classification(&classification).ok_or_else(|| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
            )
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?,
        );
    }
    Ok(keys)
}

fn prepare_metadata_refresh(
    connection: &Connection,
    access: &fasti_application::RequestAccessContext,
    record_id: RecordId,
    provider_id: &MetadataProviderId,
    field_groups: &[MetadataFieldGroup],
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<PreparedMetadataRefresh> {
    let workspace_id = access.workspace_id();
    let profile_id = access.profile_id();
    let grain = map_sql(
        connection
            .query_row(
                r#"
                SELECT grain FROM records
                WHERE workspace_id = ?1 AND record_id = ?2 AND status = 'active'
                "#,
                params![workspace_id.to_string(), record_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional(),
        capability,
        correlation_id,
    )?
    .ok_or_else(|| Box::new(FastiProblem::record_not_found(capability, correlation_id)))?
    .parse::<fasti_domain::Grain>()
    .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let mapping = provider_identity_mapping_for_grain(provider_id.as_str(), grain)
        .ok_or_else(|| invalid_provider_metadata(capability, correlation_id))?;
    let mut statement = map_sql(
        connection.prepare(
            r#"
            SELECT value FROM external_identifiers
            WHERE workspace_id = ?1 AND record_id = ?2
              AND namespace = ?3 AND grain = ?4
            ORDER BY external_identifier_id
            "#,
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map(
            params![
                workspace_id.to_string(),
                record_id.to_string(),
                mapping.namespace(),
                grain.as_str()
            ],
            |row| row.get::<_, String>(0),
        ),
        capability,
        correlation_id,
    )?;
    let mut identifiers = Vec::new();
    for value in rows {
        identifiers.push(map_sql(value, capability, correlation_id)?);
    }
    if identifiers.len() != 1 {
        return Err(invalid_provider_metadata(capability, correlation_id));
    }
    let identifier = mapping
        .identifier(identifiers.remove(0))
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
    let enrichment_policy = load_enrichment_policy(
        connection,
        workspace_id,
        profile_id,
        capability,
        correlation_id,
    )?;
    if field_groups.is_empty()
        || field_groups
            .iter()
            .any(|group| !enrichment_policy.field_group_is_enabled(*group))
    {
        return Err(invalid_provider_metadata(capability, correlation_id));
    }
    let settings_fingerprint =
        enrichment_policy_fingerprint(&enrichment_policy, capability, correlation_id)?;
    Ok(PreparedMetadataRefresh::new(
        record_id,
        grain,
        identifier,
        field_groups.to_vec(),
        settings_fingerprint,
    ))
}

fn validate_refresh_commit(
    command: &CommitMetadataRefreshCommand,
    current: &PreparedMetadataRefresh,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    if current != command.prepared()
        || command.fields().len() > MAX_PROVIDER_METADATA_FIELDS
        || command.ratings().len() > 256
        || command.cache_entries().len() > 256
        || command.attribution().provider_id() != command.provider_id()
    {
        return Err(immutable_claim_conflict(capability, correlation_id));
    }
    let identifier = current.identifier();
    let mut field_keys = BTreeSet::new();
    let mut submitted_claim_groups = Vec::new();
    for field in command.fields() {
        let claim = field.claim();
        let provenance = claim.provenance();
        if !field_keys.insert(field.field_key().as_str())
            || claim
                .record_id()
                .is_some_and(|value| value != current.record_id())
            || claim
                .field_key()
                .is_some_and(|value| value != field.field_key())
            || provenance.provider_id() != Some(command.provider_id())
            || provenance.source_namespace().as_str() != identifier.namespace()
            || provenance.source_identifier() != Some(identifier.value())
        {
            return Err(immutable_claim_conflict(capability, correlation_id));
        }
        let group = metadata_field_group(field.field_key())
            .ok_or_else(|| immutable_claim_conflict(capability, correlation_id))?;
        submitted_claim_groups.push((claim.claim_id(), group));
    }
    for rating in command.ratings() {
        let provenance = rating.provenance();
        if rating.record_id() != current.record_id()
            || provenance.provider_id() != Some(command.provider_id())
            || provenance.source_namespace().as_str() != identifier.namespace()
            || provenance.source_identifier() != Some(identifier.value())
        {
            return Err(immutable_claim_conflict(capability, correlation_id));
        }
    }
    for entry in command.cache_entries() {
        let key = entry.key();
        if key.provider_id() != command.provider_id()
            || key.record_id() != current.record_id()
            || key.grain() != current.grain()
            || key.source_namespace().as_str() != identifier.namespace()
            || key.source_identifier() != identifier.value()
            || key.settings_fingerprint() != current.settings_fingerprint()
            || entry.claim_ids().iter().any(|claim_id| {
                submitted_claim_groups
                    .iter()
                    .find_map(|(submitted_id, group)| (submitted_id == claim_id).then_some(*group))
                    != Some(key.field_group())
            })
        {
            return Err(immutable_claim_conflict(capability, correlation_id));
        }
    }
    Ok(())
}

fn provider_state_matches(
    connection: &Connection,
    workspace_id: WorkspaceId,
    expected: &ProviderCapabilityState,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<bool> {
    let matches: i64 = map_sql(
        connection.query_row(
            r#"
            SELECT COUNT(*) FROM provider_capability_states
            WHERE workspace_id = ?1
              AND provider_id = ?2
              AND capability_id = ?3
              AND capability_status = ?4
              AND capability_version = ?5
              AND credential_requirement = ?6
              AND credential_reference IS ?7
              AND credential_status = ?8
              AND configuration_digest = ?9
            "#,
            params![
                workspace_id.to_string(),
                expected.provider_id().as_str(),
                expected.capability_id().as_str(),
                expected.capability_status().as_str(),
                expected.capability_version() as i64,
                expected.credential_requirement().as_str(),
                expected.credential_reference().map(|value| value.as_str()),
                expected.credential_status().as_str(),
                expected.configuration_digest().as_str(),
            ],
            |row| row.get(0),
        ),
        capability,
        correlation_id,
    )?;
    Ok(matches == 1)
}

impl MetadataRefreshPersistencePort for SqliteKernel {
    fn authorize_and_prepare_refresh(
        &self,
        command: PrepareMetadataRefreshCommand,
    ) -> ApplicationResult<PreparedMetadataRefresh> {
        let capability = CapabilityKey::RefreshMetadataClaims;
        let correlation_id = command.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Deferred),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        let prepared = prepare_metadata_refresh(
            &transaction,
            command.access(),
            command.record_id(),
            command.provider_id(),
            command.field_groups(),
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(prepared)
    }

    fn authorize_and_read_cached_refresh(
        &self,
        command: ReadCachedMetadataRefreshCommand,
    ) -> ApplicationResult<Option<RefreshMetadataClaimsOutcome>> {
        let capability = CapabilityKey::RefreshMetadataClaims;
        let correlation_id = command.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Deferred),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        let workspace_id = command.access().workspace_id();
        let profile_id = command.access().profile_id();
        let prepared = command.prepared();
        let current_grain = map_sql(
            transaction
                .query_row(
                    r#"
                    SELECT grain FROM records
                    WHERE workspace_id = ?1 AND record_id = ?2 AND status = 'active'
                    "#,
                    params![workspace_id.to_string(), prepared.record_id().to_string()],
                    |row| row.get::<_, String>(0),
                )
                .optional(),
            capability,
            correlation_id,
        )?
        .ok_or_else(|| Box::new(FastiProblem::record_not_found(capability, correlation_id)))?
        .parse::<fasti_domain::Grain>()
        .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
        let identifier_count: i64 = map_sql(
            transaction.query_row(
                r#"
                SELECT COUNT(*) FROM external_identifiers
                WHERE workspace_id = ?1 AND record_id = ?2
                  AND namespace = ?3 AND grain = ?4 AND value = ?5
                "#,
                params![
                    workspace_id.to_string(),
                    prepared.record_id().to_string(),
                    prepared.identifier().namespace(),
                    prepared.identifier().grain().as_str(),
                    prepared.identifier().value()
                ],
                |row| row.get(0),
            ),
            capability,
            correlation_id,
        )?;
        let current_policy = load_enrichment_policy(
            &transaction,
            workspace_id,
            profile_id,
            capability,
            correlation_id,
        )?;
        let current_fingerprint =
            enrichment_policy_fingerprint(&current_policy, capability, correlation_id)?;
        if current_grain != prepared.grain()
            || prepared.identifier().grain() != prepared.grain()
            || identifier_count != 1
            || &current_fingerprint != prepared.settings_fingerprint()
        {
            return Err(immutable_claim_conflict(capability, correlation_id));
        }
        if command.cache_keys().is_empty() {
            map_sql(transaction.commit(), capability, correlation_id)?;
            return Ok(None);
        }
        let provider_id = command.cache_keys()[0].provider_id();
        let refreshed = prepare_metadata_refresh(
            &transaction,
            command.access(),
            prepared.record_id(),
            provider_id,
            prepared.field_groups(),
            capability,
            correlation_id,
        )?;
        if &refreshed != prepared {
            return Err(immutable_claim_conflict(capability, correlation_id));
        }
        let read_at = now();
        let mut entries = Vec::with_capacity(command.cache_keys().len());
        let mut referenced_claim_ids = Vec::new();
        for key in command.cache_keys() {
            if key.provider_id() != provider_id
                || key.record_id() != prepared.record_id()
                || key.grain() != prepared.grain()
                || key.source_namespace().as_str() != prepared.identifier().namespace()
                || key.source_identifier() != prepared.identifier().value()
                || key.settings_fingerprint() != prepared.settings_fingerprint()
                || key.purpose() != MetadataCachePurpose::MetadataEnrichment
                || key.classification() != MetadataDataClassification::Public
            {
                return Err(immutable_claim_conflict(capability, correlation_id));
            }
            let Some(entry) = load_metadata_cache_entry(
                &transaction,
                workspace_id,
                key,
                capability,
                correlation_id,
            )?
            else {
                map_sql(transaction.commit(), capability, correlation_id)?;
                return Ok(None);
            };
            let state = entry.read_state(
                read_at,
                MetadataCachePurpose::MetadataEnrichment,
                MetadataDataClassification::Public,
                false,
            );
            if state != MetadataCacheReadState::Fresh {
                map_sql(transaction.commit(), capability, correlation_id)?;
                return Ok(None);
            }
            for claim_id in entry.claim_ids() {
                if !referenced_claim_ids.contains(claim_id) {
                    referenced_claim_ids.push(*claim_id);
                }
            }
            entries.push(MetadataCacheReadView::new(entry, state));
        }
        let mut field_targets = BTreeSet::new();
        let mut rating_ids = Vec::new();
        for claim_id in &referenced_claim_ids {
            let target = map_sql(
                transaction
                    .query_row(
                        r#"
                        SELECT registered.claim_kind, provenance.field_key
                        FROM metadata_claims registered
                        LEFT JOIN metadata_claim_provenance provenance
                          ON provenance.claim_id = registered.claim_id
                        WHERE registered.workspace_id = ?1
                          AND registered.record_id = ?2
                          AND registered.claim_id = ?3
                        "#,
                        params![
                            workspace_id.to_string(),
                            prepared.record_id().to_string(),
                            claim_id.to_string()
                        ],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
                    )
                    .optional(),
                capability,
                correlation_id,
            )?
            .ok_or_else(|| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
            match target {
                (kind, Some(field_key)) if kind == "field" => {
                    field_targets.insert(FieldKey::try_new(field_key).map_err(|_| {
                        Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                    })?);
                }
                (kind, None) if kind == "rating" => {
                    if !rating_ids.contains(claim_id) {
                        rating_ids.push(*claim_id);
                    }
                }
                _ => {
                    return Err(Box::new(FastiProblem::integrity_failed(
                        capability,
                        correlation_id,
                    )))
                }
            }
        }
        let projection_policy = current_policy.projection_policy();
        let mut field_views = Vec::new();
        let mut projections = Vec::new();
        for field_key in field_targets {
            let claims = load_field_claims(
                &transaction,
                workspace_id,
                prepared.record_id(),
                &field_key,
                capability,
                correlation_id,
            )?;
            for claim in claims
                .iter()
                .filter(|claim| referenced_claim_ids.contains(&claim.claim_id()))
            {
                field_views.push(fasti_application::FieldClaimView::new(
                    claim.clone(),
                    claim.status_at(read_at),
                ));
            }
            let override_ = load_profile_field_override(
                &transaction,
                workspace_id,
                profile_id,
                prepared.record_id(),
                &field_key,
                capability,
                correlation_id,
            )?;
            let resolved =
                resolve_profile_field(override_.as_ref(), &claims, &[], projection_policy, read_at)
                    .map_err(|_| {
                        Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                    })?;
            projections.push(
                MetadataProjection::try_new(
                    profile_id,
                    prepared.record_id(),
                    field_key,
                    resolved,
                    ReceivedAt::from_application_clock(read_at),
                )
                .map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
            );
        }
        let mut rating_views = Vec::new();
        for claim in load_rating_claims(
            &transaction,
            workspace_id,
            prepared.record_id(),
            capability,
            correlation_id,
        )? {
            if rating_ids.contains(&claim.claim_id()) {
                rating_views.push(RatingClaimView::new(
                    claim.clone(),
                    claim.status_at(read_at),
                ));
            }
        }
        if field_views.len() + rating_views.len() != referenced_claim_ids.len() {
            return Err(Box::new(FastiProblem::integrity_failed(
                capability,
                correlation_id,
            )));
        }
        let attributions =
            load_record_attributions(&transaction, workspace_id, capability, correlation_id)?
                .into_iter()
                .filter(|attribution| attribution.provider_id() == provider_id)
                .collect();
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(Some(RefreshMetadataClaimsOutcome::new(
            field_views,
            rating_views,
            projections,
            entries,
            attributions,
        )))
    }

    fn authorize_and_mark_refresh_unavailable(
        &self,
        command: MarkMetadataRefreshUnavailableCommand,
    ) -> ApplicationResult<()> {
        let capability = CapabilityKey::RefreshMetadataClaims;
        let correlation_id = command.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        let workspace_id = command.access().workspace_id();
        let current = prepare_metadata_refresh(
            &transaction,
            command.access(),
            command.prepared().record_id(),
            command.provider_id(),
            command.prepared().field_groups(),
            capability,
            correlation_id,
        )?;
        if &current != command.prepared() {
            return Err(immutable_claim_conflict(capability, correlation_id));
        }
        let mut after_claim_id = String::new();
        let mut transitions = Vec::new();
        loop {
            let mut statement = map_sql(
                transaction.prepare(
                    r#"
                SELECT registered.claim_id,
                       COALESCE((
                           SELECT MAX(lifecycle.sequence)
                           FROM metadata_claim_lifecycle_events lifecycle
                           WHERE lifecycle.claim_id = registered.claim_id
                       ), 0),
                       COALESCE((
                           SELECT lifecycle.status
                           FROM metadata_claim_lifecycle_events lifecycle
                           WHERE lifecycle.claim_id = registered.claim_id
                           ORDER BY lifecycle.sequence DESC LIMIT 1
                       ), CASE registered.claim_kind
                           WHEN 'field' THEN (
                               SELECT provenance.initial_status
                               FROM metadata_claim_provenance provenance
                               WHERE provenance.claim_id = registered.claim_id
                           )
                           WHEN 'rating' THEN (
                               SELECT rating.initial_status
                               FROM metadata_rating_claims rating
                               WHERE rating.claim_id = registered.claim_id
                           )
                       END)
                FROM metadata_claims registered
                WHERE registered.workspace_id = ?1
                  AND registered.record_id = ?2
                  AND (
                    EXISTS (
                        SELECT 1 FROM metadata_claim_provenance provenance
                        WHERE provenance.claim_id = registered.claim_id
                          AND provenance.provider_id = ?3
                    ) OR EXISTS (
                        SELECT 1 FROM metadata_rating_claims rating
                        WHERE rating.claim_id = registered.claim_id
                          AND rating.provider_id = ?3
                    )
                  )
                  AND registered.claim_id > ?4
                ORDER BY registered.claim_id
                LIMIT 512
                "#,
                ),
                capability,
                correlation_id,
            )?;
            let rows = map_sql(
                statement.query_map(
                    params![
                        workspace_id.to_string(),
                        current.record_id().to_string(),
                        command.provider_id().as_str(),
                        after_claim_id
                    ],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                ),
                capability,
                correlation_id,
            )?;
            let mut page = Vec::new();
            for row in rows {
                page.push(map_sql(row, capability, correlation_id)?);
            }
            drop(statement);
            if page.is_empty() {
                break;
            }
            after_claim_id = page.last().expect("non-empty page").0.clone();
            for (claim_id, sequence, status) in page {
                let status = parse_claim_status(&status)
                    .ok_or_else(|| immutable_claim_conflict(capability, correlation_id))?;
                if matches!(status, FieldClaimStatus::Fresh | FieldClaimStatus::Stale) {
                    transitions.push((
                        claim_id
                            .parse::<MetadataClaimId>()
                            .map_err(|_| immutable_claim_conflict(capability, correlation_id))?,
                        u32::try_from(
                            sequence.checked_add(1).ok_or_else(|| {
                                immutable_claim_conflict(capability, correlation_id)
                            })?,
                        )
                        .map_err(|_| immutable_claim_conflict(capability, correlation_id))?,
                        status,
                    ));
                }
            }
        }
        let occurred_at = ReceivedAt::from_application_clock(now());
        for (claim_id, sequence, previous_status) in transitions {
            let event = FieldClaimLifecycleEvent::try_new(
                claim_id,
                sequence,
                previous_status,
                FieldClaimStatus::Unavailable,
                occurred_at,
                None,
            )
            .map_err(|_| immutable_claim_conflict(capability, correlation_id))?;
            append_field_claim_lifecycle_event(
                &transaction,
                workspace_id,
                &event,
                capability,
                correlation_id,
            )?;
        }
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(())
    }

    fn authorize_and_commit_refresh(
        &self,
        command: CommitMetadataRefreshCommand,
    ) -> ApplicationResult<RefreshMetadataClaimsOutcome> {
        let capability = CapabilityKey::RefreshMetadataClaims;
        let correlation_id = command.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        let workspace_id = command.access().workspace_id();
        let profile_id = command.access().profile_id();
        let current = prepare_metadata_refresh(
            &transaction,
            command.access(),
            command.prepared().record_id(),
            command.provider_id(),
            command.prepared().field_groups(),
            capability,
            correlation_id,
        )?;
        if !provider_state_matches(
            &transaction,
            workspace_id,
            command.expected_provider_state(),
            capability,
            correlation_id,
        )? {
            return Err(immutable_claim_conflict(capability, correlation_id));
        }
        validate_refresh_commit(&command, &current, capability, correlation_id)?;
        let write_at = now();
        let policy = load_projection_policy(
            &transaction,
            workspace_id,
            profile_id,
            capability,
            correlation_id,
        )?;
        let mut field_views = Vec::with_capacity(command.fields().len());
        let mut projections = Vec::with_capacity(command.fields().len());
        for field in command.fields() {
            write_field_claim(
                &transaction,
                workspace_id,
                current.record_id(),
                field.field_key(),
                field.claim(),
                capability,
                correlation_id,
            )?;
            field_views.push(fasti_application::FieldClaimView::new(
                field.claim().clone(),
                field.claim().status_at(write_at),
            ));
            let claims = load_field_claims(
                &transaction,
                workspace_id,
                current.record_id(),
                field.field_key(),
                capability,
                correlation_id,
            )?;
            let override_ = load_profile_field_override(
                &transaction,
                workspace_id,
                profile_id,
                current.record_id(),
                field.field_key(),
                capability,
                correlation_id,
            )?;
            let resolved =
                resolve_profile_field(override_.as_ref(), &claims, &[], &policy, write_at)
                    .map_err(|_| {
                        Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                    })?;
            let projection = MetadataProjection::try_new(
                profile_id,
                current.record_id(),
                field.field_key().clone(),
                resolved,
                ReceivedAt::from_application_clock(write_at),
            )
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
            write_metadata_projection(
                &transaction,
                workspace_id,
                &projection,
                capability,
                correlation_id,
            )?;
            projections.push(projection);
        }
        let mut rating_views = Vec::with_capacity(command.ratings().len());
        for rating in command.ratings() {
            write_rating_claim(
                &transaction,
                workspace_id,
                rating,
                capability,
                correlation_id,
            )?;
            rating_views.push(RatingClaimView::new(
                rating.clone(),
                rating.status_at(write_at),
            ));
        }
        let mut cache_views = Vec::with_capacity(command.cache_entries().len());
        for entry in command.cache_entries() {
            write_metadata_cache_entry(
                &transaction,
                workspace_id,
                entry,
                capability,
                correlation_id,
            )?;
            cache_views.push(MetadataCacheReadView::new(
                entry.clone(),
                entry.read_state(
                    write_at,
                    entry.key().purpose(),
                    MetadataDataClassification::Internal,
                    false,
                ),
            ));
        }
        write_metadata_attribution(
            &transaction,
            workspace_id,
            command.attribution(),
            capability,
            correlation_id,
        )?;
        let attribution = command.attribution().clone();
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(RefreshMetadataClaimsOutcome::new(
            field_views,
            rating_views,
            projections,
            cache_views,
            vec![attribution],
        ))
    }
}

impl MetadataProjectionPort for SqliteKernel {
    fn authorize_and_read_projection(
        &self,
        query: ReadMetadataProjectionQuery,
    ) -> ApplicationResult<MetadataProjectionView> {
        let capability = CapabilityKey::ReadMetadataProjection;
        let correlation_id = query.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Deferred),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, query.access(), correlation_id)?;
        let workspace_id = query.access().workspace_id();
        let profile_id = query.access().profile_id();
        let record_exists = map_sql(
            transaction
                .query_row(
                    r#"
                    SELECT 1 FROM records
                    WHERE workspace_id = ?1 AND record_id = ?2 AND status = 'active'
                    "#,
                    params![workspace_id.to_string(), query.record_id().to_string()],
                    |_| Ok(()),
                )
                .optional(),
            capability,
            correlation_id,
        )?
        .is_some();
        if !record_exists {
            return Err(Box::new(FastiProblem::record_not_found(
                capability,
                correlation_id,
            )));
        }
        let policy = load_enrichment_policy(
            &transaction,
            workspace_id,
            profile_id,
            capability,
            correlation_id,
        )?;
        let mut statement = map_sql(
            transaction.prepare(
                r#"
                SELECT field_key FROM (
                    SELECT field_key FROM metadata_claim_provenance
                    WHERE workspace_id = ?1 AND record_id = ?2
                    UNION
                    SELECT field_key FROM metadata_profile_field_overrides
                    WHERE workspace_id = ?1 AND profile_id = ?3 AND record_id = ?2
                )
                ORDER BY field_key
                LIMIT 256
                "#,
            ),
            capability,
            correlation_id,
        )?;
        let field_rows = map_sql(
            statement.query_map(
                params![
                    workspace_id.to_string(),
                    query.record_id().to_string(),
                    profile_id.to_string()
                ],
                |row| row.get::<_, String>(0),
            ),
            capability,
            correlation_id,
        )?;
        let mut field_keys = Vec::new();
        for field_key in field_rows {
            let field_key = FieldKey::try_new(map_sql(field_key, capability, correlation_id)?)
                .map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?;
            if metadata_field_group(&field_key)
                .is_some_and(|group| policy.field_group_is_enabled(group))
            {
                field_keys.push(field_key);
            }
        }
        drop(statement);

        let read_at = now();
        let projected_at = ReceivedAt::from_application_clock(read_at);
        let mut fields = Vec::with_capacity(field_keys.len());
        for field_key in field_keys {
            let claims = load_field_claims(
                &transaction,
                workspace_id,
                query.record_id(),
                &field_key,
                capability,
                correlation_id,
            )?;
            let override_ = load_profile_field_override(
                &transaction,
                workspace_id,
                profile_id,
                query.record_id(),
                &field_key,
                capability,
                correlation_id,
            )?;
            let resolved = resolve_profile_field(
                override_.as_ref(),
                &claims,
                &[],
                policy.projection_policy(),
                read_at,
            )
            .map_err(|_| Box::new(FastiProblem::integrity_failed(capability, correlation_id)))?;
            fields.push(
                MetadataProjection::try_new(
                    profile_id,
                    query.record_id(),
                    field_key,
                    resolved,
                    projected_at,
                )
                .map_err(|_| {
                    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
                })?,
            );
        }
        let ratings: Vec<RatingClaimView> = load_rating_claims(
            &transaction,
            workspace_id,
            query.record_id(),
            capability,
            correlation_id,
        )?
        .into_iter()
        .map(|claim| RatingClaimView::new(claim.clone(), claim.status_at(read_at)))
        .collect();
        let mut cache_entries = Vec::new();
        let settings_fingerprint =
            enrichment_policy_fingerprint(&policy, capability, correlation_id)?;
        for key in load_record_cache_keys(
            &transaction,
            workspace_id,
            query.record_id(),
            &settings_fingerprint,
            capability,
            correlation_id,
        )? {
            if let Some(entry) = load_metadata_cache_entry(
                &transaction,
                workspace_id,
                &key,
                capability,
                correlation_id,
            )? {
                let state = entry.read_state(
                    read_at,
                    key.purpose(),
                    MetadataDataClassification::Internal,
                    query.offline(),
                );
                cache_entries.push(MetadataCacheReadView::new(entry, state));
            }
        }
        let mut represented_providers = BTreeSet::new();
        for projection in &fields {
            if let Some(provider_id) = projection
                .resolved_field()
                .provenance()
                .and_then(|provenance| provenance.claim_provenance().provider_id())
            {
                represented_providers.insert(provider_id.as_str().to_owned());
            }
        }
        for rating in &ratings {
            if let Some(provider_id) = rating.claim().provenance().provider_id() {
                represented_providers.insert(provider_id.as_str().to_owned());
            }
        }
        let attributions =
            load_record_attributions(&transaction, workspace_id, capability, correlation_id)?
                .into_iter()
                .filter(|attribution| {
                    represented_providers.contains(attribution.provider_id().as_str())
                })
                .collect();
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(MetadataProjectionView::new(
            profile_id,
            query.record_id(),
            policy,
            fields,
            ratings,
            cache_entries,
            attributions,
        ))
    }

    fn authorize_and_configure_projection(
        &self,
        command: ConfigureMetadataProjectionCommand,
    ) -> ApplicationResult<ConfigureMetadataProjectionOutcome> {
        let capability = CapabilityKey::ConfigureMetadataProjection;
        let correlation_id = command.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        let workspace_id = command.access().workspace_id();
        let profile_id = command.access().profile_id();
        if command.projection_policy().profile_id() != profile_id
            || command.override_mutations().len() > 256
        {
            return Err(invalid_provider_metadata(capability, correlation_id));
        }
        let prior_policy = load_enrichment_policy(
            &transaction,
            workspace_id,
            profile_id,
            capability,
            correlation_id,
        )?;
        let policy = EnrichmentPolicy::new(
            command.projection_policy().clone(),
            command.region().cloned(),
            command.enabled_field_groups().to_vec(),
        );
        let policy_changed = prior_policy != policy;
        let prior_fingerprint =
            enrichment_policy_fingerprint(&prior_policy, capability, correlation_id)?;
        write_enrichment_policy(
            &transaction,
            workspace_id,
            &policy,
            capability,
            correlation_id,
        )?;
        let mutation_time = ReceivedAt::from_application_clock(now());
        for mutation in command.override_mutations() {
            match mutation {
                MetadataOverrideMutation::Set {
                    record_id,
                    field_key,
                    value,
                } => {
                    let override_ = ProfileFieldOverride::try_new(
                        profile_id,
                        *record_id,
                        field_key.clone(),
                        value,
                        mutation_time,
                    )
                    .map_err(|_| invalid_provider_metadata(capability, correlation_id))?;
                    write_profile_field_override(
                        &transaction,
                        workspace_id,
                        &override_,
                        capability,
                        correlation_id,
                    )?;
                }
                MetadataOverrideMutation::Clear {
                    record_id,
                    field_key,
                } => {
                    map_sql(
                        transaction.execute(
                            r#"
                            DELETE FROM metadata_profile_field_overrides
                            WHERE workspace_id = ?1 AND profile_id = ?2
                              AND record_id = ?3 AND field_key = ?4
                            "#,
                            params![
                                workspace_id.to_string(),
                                profile_id.to_string(),
                                record_id.to_string(),
                                field_key.as_str()
                            ],
                        ),
                        capability,
                        correlation_id,
                    )?;
                }
            }
        }
        let invalidated_cache_entries = if policy_changed {
            map_sql(
                transaction.execute(
                    r#"
                    UPDATE metadata_cache_entries
                    SET invalidation_reason = 'projection_policy_changed',
                        invalidated_at = ?1,
                        updated_at = ?1
                    WHERE workspace_id = ?2
                      AND purpose = 'display_projection'
                      AND settings_fingerprint = ?3
                      AND invalidation_reason IS NULL
                    "#,
                    params![
                        timestamp(now()),
                        workspace_id.to_string(),
                        prior_fingerprint.to_string()
                    ],
                ),
                capability,
                correlation_id,
            )?
        } else {
            0
        };
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(ConfigureMetadataProjectionOutcome::new(
            policy,
            u32::try_from(invalidated_cache_entries).map_err(|_| {
                Box::new(FastiProblem::integrity_failed(capability, correlation_id))
            })?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestNode;
    use fasti_application::{
        provider_identity_mapping, ApplyProviderMetadataCommand, CommitMetadataRefreshCommand,
        ConfigurationDigest, ConfigureMetadataProjectionCommand, CreateProviderRecordCommand,
        CreateRecordCommand, CredentialReference, CredentialRequirement, IdentityPort,
        ListRecordsQuery, MarkMetadataRefreshUnavailableCommand, MetadataProjectionPort,
        MetadataRefreshPersistencePort, PrepareMetadataRefreshCommand, PreparedMetadataRefresh,
        ProviderCapabilityId, ProviderCapabilityState, ProviderCapabilityStatus,
        ProviderCheckMetadata, ProviderCredentialStatus, ProviderId, ProviderIdentityMapping,
        ProviderMetadataField, ProviderMetadataPort, ProviderStatePort,
        RegisterNamespaceDefinitionCommand, GOOGLE_BOOKS_PROVIDER_ID, TMDB_PROVIDER_ID,
    };
    use fasti_domain::{
        Grain, LastKnownGoodPolicy, MetadataCacheReadState, MetadataLocale,
        MetadataProjectionPolicy, MetadataProviderId, ProfileFieldOverride, ProfileId, ReceivedAt,
        TITLE_FIELD_KEY,
    };

    fn field_key(value: &str) -> FieldKey {
        FieldKey::try_new(value).expect("valid field key")
    }

    fn ns(value: &str) -> NamespaceKey {
        NamespaceKey::try_new(value).expect("valid namespace")
    }

    fn received(seconds: i64) -> ReceivedAt {
        use chrono::TimeZone;
        ReceivedAt::from_application_clock(
            chrono::Utc
                .timestamp_opt(seconds, 0)
                .single()
                .expect("valid instant"),
        )
    }

    fn digest(byte: &str) -> Sha256Digest {
        format!("sha256:{}", byte.repeat(64))
            .parse()
            .expect("valid digest")
    }

    fn provenance(source_identifier: &str) -> FieldClaimProvenance {
        FieldClaimProvenance::try_new(
            MetadataProviderId::try_new("tmdb").expect("provider"),
            ns("tmdb"),
            source_identifier,
            Some(MetadataLocale::try_new("en-IE").expect("locale")),
            Some(MetadataRegion::try_new("IE").expect("region")),
            Some("2026-08-30".to_owned()),
            digest("a"),
        )
        .expect("complete provenance")
    }

    fn a_record(node: &TestNode) -> RecordId {
        node.kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Film,
            ))
            .expect("create record")
            .record_id()
    }

    #[test]
    fn metadata_scope_triggers_reject_cross_workspace_rows() {
        let node = TestNode::new();
        let other_workspace = WorkspaceId::new_v7();
        let other_profile = ProfileId::new_v7();
        let other_record = RecordId::new_v7();
        let connection = node.kernel.inner.connection.lock().expect("connection");
        connection
            .execute(
                "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                params![other_workspace.to_string(), timestamp(now())],
            )
            .expect("other workspace");
        connection
            .execute(
                "INSERT INTO profiles(profile_id, workspace_id, created_at) VALUES (?1, ?2, ?3)",
                params![
                    other_profile.to_string(),
                    other_workspace.to_string(),
                    timestamp(now())
                ],
            )
            .expect("other profile");
        connection
            .execute(
                "INSERT INTO records(record_id, workspace_id, grain, status, created_at) VALUES (?1, ?2, 'film', 'active', ?3)",
                params![
                    other_record.to_string(),
                    other_workspace.to_string(),
                    timestamp(now())
                ],
            )
            .expect("other record");

        assert!(connection
            .execute(
                "INSERT INTO metadata_claims(claim_id, workspace_id, record_id, claim_kind, created_at) VALUES (?1, ?2, ?3, 'field', ?4)",
                params![
                    MetadataClaimId::new_v7().to_string(),
                    node.access.workspace_id().to_string(),
                    other_record.to_string(),
                    timestamp(now())
                ],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO metadata_field_claims(workspace_id, record_id, field_key, source, value, fetched_at, created_at) VALUES (?1, ?2, 'core.title', 'tmdb', 'crossed', ?3, ?3)",
                params![
                    node.access.workspace_id().to_string(),
                    other_record.to_string(),
                    timestamp(now())
                ],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO metadata_projection_policies(workspace_id, profile_id, enabled_field_groups, allow_english_fallback, last_known_good_policy, updated_at) VALUES (?1, ?2, '[]', 1, 'allow', ?3)",
                params![
                    node.access.workspace_id().to_string(),
                    other_profile.to_string(),
                    timestamp(now())
                ],
            )
            .is_err());
        assert!(connection
            .execute(
                "INSERT INTO metadata_profile_field_overrides(workspace_id, profile_id, record_id, field_key, value, created_at, updated_at, origin) VALUES (?1, ?2, ?3, 'core.title', 'crossed', ?4, ?4, 'user')",
                params![
                    node.access.workspace_id().to_string(),
                    node.access.profile_id().to_string(),
                    other_record.to_string(),
                    timestamp(now())
                ],
            )
            .is_err());
    }

    fn register_mapping(node: &TestNode, mapping: ProviderIdentityMapping) {
        node.kernel
            .register_namespace_definition(RegisterNamespaceDefinitionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                mapping.namespace_definition().expect("provider namespace"),
            ))
            .expect("register namespace");
    }

    fn provider_field(source: &str, key: &str, value: &str) -> ProviderMetadataField {
        ProviderMetadataField::new(
            field_key(key),
            FieldClaim::try_new(ns(source), value, None, received(100), None)
                .expect("provider claim"),
        )
    }

    fn refresh_fixture(node: &TestNode) -> (RecordId, MetadataProviderId, PreparedMetadataRefresh) {
        let mapping =
            provider_identity_mapping(TMDB_PROVIDER_ID, "movie").expect("TMDB movie mapping");
        register_mapping(node, mapping);
        let record_id = a_record(node);
        node.kernel
            .apply_provider_metadata(ApplyProviderMetadataCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record_id,
                mapping.identifier("438631").expect("TMDB identifier"),
                vec![provider_field(
                    mapping.namespace(),
                    "core.overview",
                    "Identity fixture",
                )],
            ))
            .expect("attach TMDB identity");
        node.kernel
            .authorize_and_configure_projection(ConfigureMetadataProjectionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                MetadataProjectionPolicy::default_for_profile(node.access.profile_id()),
                None,
                vec![MetadataFieldGroup::BasicInfo],
                Vec::new(),
            ))
            .expect("enable basic metadata");
        let provider_id = MetadataProviderId::try_new(TMDB_PROVIDER_ID).expect("provider ID");
        let prepared = node
            .kernel
            .authorize_and_prepare_refresh(PrepareMetadataRefreshCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record_id,
                provider_id.clone(),
                vec![MetadataFieldGroup::BasicInfo],
            ))
            .expect("prepare metadata refresh");
        (record_id, provider_id, prepared)
    }

    fn provider_state(version: u64) -> ProviderCapabilityState {
        ProviderCapabilityState::try_new(
            ProviderId::try_new(TMDB_PROVIDER_ID).expect("provider state ID"),
            ProviderCapabilityId::try_new("metadata.read").expect("capability ID"),
            ProviderCapabilityStatus::Available,
            version,
            CredentialRequirement::ApiKey,
            Some(CredentialReference::try_new("tmdb-api-key").expect("credential reference")),
            ProviderCredentialStatus::StoredUnverified,
            ConfigurationDigest::parse("a".repeat(64)).expect("configuration digest"),
            ProviderCheckMetadata::never_run(),
            ProviderCheckMetadata::never_run(),
        )
        .expect("provider state")
    }

    #[test]
    fn provider_record_creation_is_atomic_and_safe_to_retry_after_an_ambiguous_response() {
        let node = TestNode::new();
        let mapping = provider_identity_mapping(GOOGLE_BOOKS_PROVIDER_ID, "book")
            .expect("Google Books mapping");
        register_mapping(&node, mapping);
        let command = || {
            CreateProviderRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                mapping.grain(),
                mapping.identifier("book-1").expect("identifier"),
                vec![provider_field(
                    mapping.namespace(),
                    TITLE_FIELD_KEY,
                    "A real provider title",
                )],
            )
        };
        let outcome = node
            .kernel
            .create_provider_record(command())
            .expect("create enriched record");
        let retry = node
            .kernel
            .create_provider_record(command())
            .expect("retry returns the existing record");
        assert_eq!(retry.record_id(), outcome.record_id());

        let records = node
            .kernel
            .list_records(ListRecordsQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
            ))
            .expect("list records")
            .into_records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].record_id(), outcome.record_id());
        assert_eq!(records[0].title().value(), Some("A real provider title"));
        assert_eq!(records[0].identifiers().len(), 1);
        assert_eq!(records[0].identifiers()[0].value(), "book-1");
    }

    #[test]
    fn invalid_provider_fields_roll_back_the_new_record() {
        let node = TestNode::new();
        let mapping = provider_identity_mapping(GOOGLE_BOOKS_PROVIDER_ID, "book")
            .expect("Google Books mapping");
        register_mapping(&node, mapping);
        let identifier = mapping.identifier("book-1").expect("identifier");
        let result = node
            .kernel
            .create_provider_record(CreateProviderRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                mapping.grain(),
                identifier,
                vec![provider_field("tmdb", TITLE_FIELD_KEY, "Wrong source")],
            ));
        assert!(result.is_err());
        assert!(node
            .kernel
            .list_records(ListRecordsQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
            ))
            .expect("list records")
            .into_records()
            .is_empty());
    }

    #[test]
    fn provider_refresh_attaches_identity_and_metadata_together() {
        let node = TestNode::new();
        let record_id = a_record(&node);
        let mapping =
            provider_identity_mapping(TMDB_PROVIDER_ID, "movie").expect("TMDB movie mapping");
        register_mapping(&node, mapping);

        let identifier = || mapping.identifier("438631").expect("identifier");
        let invalid = node
            .kernel
            .apply_provider_metadata(ApplyProviderMetadataCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record_id,
                identifier(),
                vec![provider_field("other", TITLE_FIELD_KEY, "Wrong source")],
            ));
        assert!(invalid.is_err());

        node.kernel
            .apply_provider_metadata(ApplyProviderMetadataCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record_id,
                identifier(),
                vec![provider_field(mapping.namespace(), TITLE_FIELD_KEY, "Dune")],
            ))
            .expect("refresh provider metadata");

        let records = node
            .kernel
            .list_records(ListRecordsQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
            ))
            .expect("list records")
            .into_records();
        assert_eq!(records[0].title().value(), Some("Dune"));
        assert_eq!(records[0].identifiers().len(), 1);
        assert_eq!(records[0].identifiers()[0].value(), "438631");
    }

    #[test]
    fn claims_and_overrides_round_trip_through_persistence() {
        let node = TestNode::new();
        let record_id = a_record(&node);
        let key = field_key("core.title");
        let claim = FieldClaim::try_new(ns("tmdb"), "Example Title", None, received(100), None)
            .expect("valid claim");
        let override_ = FieldOverride::try_new("My Title", received(200)).expect("valid override");
        let capability = CapabilityKey::ListRecords;
        let correlation_id = RequestCorrelationId::new_v7();

        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            write_field_claim(
                &connection,
                node.access.workspace_id(),
                record_id,
                &key,
                &claim,
                capability,
                correlation_id,
            )
            .expect("write claim");
            write_field_override(
                &connection,
                node.access.workspace_id(),
                record_id,
                &key,
                &override_,
                capability,
                correlation_id,
            )
            .expect("write override");
        }

        let connection = node.kernel.inner.connection.lock().expect("connection");
        let claims = load_field_claims(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            capability,
            correlation_id,
        )
        .expect("load claims");
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].claim_id(), claim.claim_id());
        assert_eq!(claims[0].record_id(), Some(record_id));
        assert_eq!(claims[0].field_key(), Some(&key));
        assert_eq!(claims[0].value(), claim.value());
        assert_eq!(claims[0].provenance(), claim.provenance());

        let loaded_override = load_field_override(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            capability,
            correlation_id,
        )
        .expect("load override")
        .expect("override present");
        assert_eq!(loaded_override, override_);
    }

    #[test]
    fn upsert_conflict_rejects_a_caller_supplied_workspace_that_does_not_own_the_record() {
        let node = TestNode::new();
        let record_id = a_record(&node);
        let key = field_key("core.title");
        let capability = CapabilityKey::ListRecords;
        let correlation_id = RequestCorrelationId::new_v7();
        let foreign_workspace = WorkspaceId::new_v7();

        let connection = node.kernel.inner.connection.lock().expect("connection");
        // A real workspace row, not just an arbitrary id: otherwise the
        // attack write fails on the workspace_id foreign key before ever
        // reaching the scope-guard trigger, and the test would pass even if
        // that trigger regressed.
        connection
            .execute(
                "INSERT INTO workspaces(workspace_id, created_at) VALUES (?1, ?2)",
                rusqlite::params![foreign_workspace.to_string(), timestamp(now())],
            )
            .expect("seed foreign workspace");
        let claim = FieldClaim::try_new(ns("tmdb"), "Original title", None, received(100), None)
            .expect("valid claim");
        write_field_claim(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            &claim,
            capability,
            correlation_id,
        )
        .expect("write claim under the owning workspace");

        // Same (record_id, field_key, source, fetched_at) conflict key, but a
        // workspace_id that doesn't own record_id -- must hit the ON CONFLICT
        // path and be rejected by the scope-guard trigger, not silently
        // overwrite the owning workspace's row.
        let attack = FieldClaim::try_new(ns("tmdb"), "Hijacked title", None, received(100), None)
            .expect("valid claim");
        let result = write_field_claim(
            &connection,
            foreign_workspace,
            record_id,
            &key,
            &attack,
            capability,
            correlation_id,
        );
        assert!(result.is_err());

        let claims = load_field_claims(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            capability,
            correlation_id,
        )
        .expect("load claims");
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].claim_id(), claim.claim_id());
        assert_eq!(claims[0].record_id(), Some(record_id));
        assert_eq!(claims[0].field_key(), Some(&key));
        assert_eq!(claims[0].value(), claim.value());

        let override_ =
            FieldOverride::try_new("Original override", received(100)).expect("valid override");
        write_field_override(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            &override_,
            capability,
            correlation_id,
        )
        .expect("write override under the owning workspace");

        let attack_override =
            FieldOverride::try_new("Hijacked override", received(200)).expect("valid override");
        let result = write_field_override(
            &connection,
            foreign_workspace,
            record_id,
            &key,
            &attack_override,
            capability,
            correlation_id,
        );
        assert!(result.is_err());

        let loaded_override = load_field_override(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            capability,
            correlation_id,
        )
        .expect("load override")
        .expect("override present");
        assert_eq!(loaded_override, override_);
    }

    #[test]
    fn a_record_with_no_claims_resolves_to_an_empty_read_not_an_error() {
        let node = TestNode::new();
        let record_id = a_record(&node);
        let key = field_key("core.title");
        let capability = CapabilityKey::ListRecords;
        let correlation_id = RequestCorrelationId::new_v7();

        let connection = node.kernel.inner.connection.lock().expect("connection");
        let claims = load_field_claims(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            capability,
            correlation_id,
        )
        .expect("load claims");
        assert!(claims.is_empty());
        let override_ = load_field_override(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            capability,
            correlation_id,
        )
        .expect("load override");
        assert_eq!(override_, None);
    }

    #[test]
    fn claims_from_different_workspaces_do_not_leak() {
        let node = TestNode::new();
        let other = TestNode::new();
        let record_id = a_record(&node);
        let key = field_key("core.title");
        let claim = FieldClaim::try_new(ns("tmdb"), "Isolated Title", None, received(100), None)
            .expect("valid claim");
        let capability = CapabilityKey::ListRecords;
        let correlation_id = RequestCorrelationId::new_v7();

        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            write_field_claim(
                &connection,
                node.access.workspace_id(),
                record_id,
                &key,
                &claim,
                capability,
                correlation_id,
            )
            .expect("write claim");
        }

        // A record ID from another workspace's kernel simply is not present
        // in `other`'s database, so this proves query scoping rather than
        // relying on cross-database ID collision.
        let other_connection = other.kernel.inner.connection.lock().expect("connection");
        let claims = load_field_claims(
            &other_connection,
            other.access.workspace_id(),
            record_id,
            &key,
            capability,
            correlation_id,
        )
        .expect("load claims from another workspace's database");
        assert!(claims.is_empty());
    }

    #[test]
    fn writing_the_same_claim_twice_is_idempotent() {
        let node = TestNode::new();
        let record_id = a_record(&node);
        let key = field_key("core.title");
        let claim = FieldClaim::try_new(ns("tmdb"), "Example Title", None, received(100), None)
            .expect("valid claim");
        let capability = CapabilityKey::ListRecords;
        let correlation_id = RequestCorrelationId::new_v7();

        let connection = node.kernel.inner.connection.lock().expect("connection");
        for _ in 0..2 {
            write_field_claim(
                &connection,
                node.access.workspace_id(),
                record_id,
                &key,
                &claim,
                capability,
                correlation_id,
            )
            .expect("write claim");
        }
        let claims = load_field_claims(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            capability,
            correlation_id,
        )
        .expect("load claims");
        assert_eq!(
            claims.len(),
            1,
            "same immutable claim retry must not duplicate"
        );
    }

    #[test]
    fn conflicting_duplicate_claim_is_rejected_without_changing_the_first_claim() {
        let node = TestNode::new();
        let record_id = a_record(&node);
        let key = field_key("core.title");
        let first = FieldClaim::try_new(ns("tmdb"), "Original", None, received(100), None)
            .expect("valid claim");
        let conflict = FieldClaim::try_new(ns("tmdb"), "Changed", None, received(100), None)
            .expect("valid claim");
        let capability = CapabilityKey::ListRecords;
        let correlation_id = RequestCorrelationId::new_v7();
        let connection = node.kernel.inner.connection.lock().expect("connection");

        write_field_claim(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            &first,
            capability,
            correlation_id,
        )
        .expect("first immutable claim");
        assert!(write_field_claim(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            &conflict,
            capability,
            correlation_id,
        )
        .is_err());

        let claims = load_field_claims(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            capability,
            correlation_id,
        )
        .expect("load immutable claim");
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].value(), "Original");
    }

    #[test]
    fn profile_overrides_and_projection_policy_are_isolated() {
        let node = TestNode::new();
        let record_id = a_record(&node);
        let key = field_key("core.title");
        let other_profile = ProfileId::new_v7();
        let connection = node.kernel.inner.connection.lock().expect("connection");
        connection
            .execute(
                "INSERT INTO profiles(profile_id, workspace_id, created_at) VALUES (?1, ?2, ?3)",
                params![
                    other_profile.to_string(),
                    node.access.workspace_id().to_string(),
                    timestamp(now())
                ],
            )
            .expect("second profile");
        let first = ProfileFieldOverride::try_new(
            node.access.profile_id(),
            record_id,
            key.clone(),
            "First profile title",
            received(100),
        )
        .expect("first override");
        let second = ProfileFieldOverride::try_new(
            other_profile,
            record_id,
            key.clone(),
            "Second profile title",
            received(100),
        )
        .expect("second override");
        for override_ in [&first, &second] {
            write_profile_field_override(
                &connection,
                node.access.workspace_id(),
                override_,
                CapabilityKey::ListRecords,
                RequestCorrelationId::new_v7(),
            )
            .expect("write profile override");
        }

        let policy = MetadataProjectionPolicy::new(
            node.access.profile_id(),
            Some(MetadataProviderId::try_new("tmdb").expect("provider")),
            Some(MetadataLocale::try_new("fr-FR").expect("locale")),
            None,
            true,
            LastKnownGoodPolicy::Deny,
        );
        write_projection_policy(
            &connection,
            node.access.workspace_id(),
            &policy,
            CapabilityKey::ListRecords,
            RequestCorrelationId::new_v7(),
        )
        .expect("write policy");

        let first_loaded = load_profile_field_override(
            &connection,
            node.access.workspace_id(),
            node.access.profile_id(),
            record_id,
            &key,
            CapabilityKey::ListRecords,
            RequestCorrelationId::new_v7(),
        )
        .expect("first profile read")
        .expect("first override");
        let second_loaded = load_profile_field_override(
            &connection,
            node.access.workspace_id(),
            other_profile,
            record_id,
            &key,
            CapabilityKey::ListRecords,
            RequestCorrelationId::new_v7(),
        )
        .expect("second profile read")
        .expect("second override");
        assert_eq!(first_loaded.value(), "First profile title");
        assert_eq!(second_loaded.value(), "Second profile title");

        assert_eq!(
            load_projection_policy(
                &connection,
                node.access.workspace_id(),
                node.access.profile_id(),
                CapabilityKey::ListRecords,
                RequestCorrelationId::new_v7(),
            )
            .expect("first profile policy"),
            policy
        );
        assert_eq!(
            load_projection_policy(
                &connection,
                node.access.workspace_id(),
                other_profile,
                CapabilityKey::ListRecords,
                RequestCorrelationId::new_v7(),
            )
            .expect("default second policy"),
            MetadataProjectionPolicy::default_for_profile(other_profile)
        );
    }

    #[test]
    fn a_later_override_replaces_the_earlier_one() {
        let node = TestNode::new();
        let record_id = a_record(&node);
        let key = field_key("core.title");
        let first = FieldOverride::try_new("First Title", received(100)).expect("valid override");
        let second = FieldOverride::try_new("Second Title", received(200)).expect("valid override");
        let capability = CapabilityKey::ListRecords;
        let correlation_id = RequestCorrelationId::new_v7();

        let connection = node.kernel.inner.connection.lock().expect("connection");
        write_field_override(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            &first,
            capability,
            correlation_id,
        )
        .expect("write first override");
        write_field_override(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            &second,
            capability,
            correlation_id,
        )
        .expect("write second override");

        let loaded = load_field_override(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            capability,
            correlation_id,
        )
        .expect("load override")
        .expect("override present");
        assert_eq!(loaded, second);
    }

    #[test]
    fn immutable_claim_rows_reject_mutation_while_lifecycle_appends() {
        let node = TestNode::new();
        let record_id = a_record(&node);
        let key = field_key("core.title");
        let claim = FieldClaim::try_new_provider(
            MetadataClaimId::new_v7(),
            record_id,
            key.clone(),
            "Immutable title",
            provenance("movie-1"),
            received(101),
            None,
            FieldClaimStatus::Fresh,
        )
        .expect("provider claim");
        let capability = CapabilityKey::ListRecords;
        let correlation_id = RequestCorrelationId::new_v7();
        let connection = node.kernel.inner.connection.lock().expect("connection");
        write_field_claim(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            &claim,
            capability,
            correlation_id,
        )
        .expect("claim");

        let event = FieldClaimLifecycleEvent::try_new(
            claim.claim_id(),
            1,
            FieldClaimStatus::Fresh,
            FieldClaimStatus::Stale,
            received(200),
            None,
        )
        .expect("lifecycle event");
        append_field_claim_lifecycle_event(
            &connection,
            node.access.workspace_id(),
            &event,
            capability,
            correlation_id,
        )
        .expect("append lifecycle");
        append_field_claim_lifecycle_event(
            &connection,
            node.access.workspace_id(),
            &event,
            capability,
            correlation_id,
        )
        .expect("identical lifecycle retry");

        for sql in [
            "UPDATE metadata_field_claims SET value = 'changed'",
            "DELETE FROM metadata_field_claims",
            "UPDATE metadata_claims SET created_at = '2026-08-30T00:00:00Z'",
            "DELETE FROM metadata_claims",
            "UPDATE metadata_claim_provenance SET initial_status = 'stale'",
            "DELETE FROM metadata_claim_provenance",
            "UPDATE metadata_claim_lifecycle_events SET status = 'fresh'",
            "DELETE FROM metadata_claim_lifecycle_events",
        ] {
            assert!(connection.execute(sql, []).is_err(), "must reject: {sql}");
        }
        let status = load_field_claims(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            capability,
            correlation_id,
        )
        .expect("effective claim");
        assert_eq!(status[0].initial_status(), FieldClaimStatus::Stale);
    }

    #[test]
    fn rating_claims_round_trip_at_fixed_point_and_are_immutable() {
        let node = TestNode::new();
        let record_id = a_record(&node);
        let claim = RatingClaim::try_new(
            MetadataClaimId::new_v7(),
            record_id,
            8_750,
            RatingScale::try_new(0, 10_000).expect("scale"),
            provenance("rating-1"),
            received(100),
            None,
            FieldClaimStatus::Fresh,
        )
        .expect("rating claim");
        let capability = CapabilityKey::ListRecords;
        let correlation_id = RequestCorrelationId::new_v7();
        let connection = node.kernel.inner.connection.lock().expect("connection");
        write_rating_claim(
            &connection,
            node.access.workspace_id(),
            &claim,
            capability,
            correlation_id,
        )
        .expect("write rating");
        write_rating_claim(
            &connection,
            node.access.workspace_id(),
            &claim,
            capability,
            correlation_id,
        )
        .expect("identical rating retry");
        let loaded = load_rating_claims(
            &connection,
            node.access.workspace_id(),
            record_id,
            capability,
            correlation_id,
        )
        .expect("load ratings");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].claim_id(), claim.claim_id());
        assert_eq!(loaded[0].value_millis(), 8_750);
        assert_eq!(loaded[0].scale(), claim.scale());
        assert!(connection
            .execute("UPDATE metadata_rating_claims SET value_millis = 9000", [],)
            .is_err());
        assert!(connection
            .execute("DELETE FROM metadata_rating_claims", [])
            .is_err());
    }

    #[test]
    fn cache_persists_only_claim_references_and_clears_one_partition() {
        use chrono::Duration;

        let node = TestNode::new();
        let record_id = a_record(&node);
        let key = field_key("core.title");
        let claim = FieldClaim::try_new_provider(
            MetadataClaimId::new_v7(),
            record_id,
            key.clone(),
            "Cached title",
            provenance("movie-cache"),
            received(100),
            None,
            FieldClaimStatus::Fresh,
        )
        .expect("claim");
        let capability = CapabilityKey::ListRecords;
        let correlation_id = RequestCorrelationId::new_v7();
        let connection = node.kernel.inner.connection.lock().expect("connection");
        write_field_claim(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            &claim,
            capability,
            correlation_id,
        )
        .expect("claim");
        let cache_key = MetadataCacheKey::try_new(
            MetadataProviderId::try_new("tmdb").expect("provider"),
            Some(3),
            record_id,
            "metadata/movie",
            Grain::Film,
            ns("tmdb"),
            "movie-cache",
            Some(MetadataLocale::try_new("en-IE").expect("locale")),
            Some(MetadataRegion::try_new("IE").expect("region")),
            MetadataFieldGroup::BasicInfo,
            digest("b"),
            digest("c"),
            1,
            MetadataCachePurpose::OfflineRead,
            "terms-v1",
            MetadataDataClassification::Internal,
        )
        .expect("cache key");
        let created = received(300);
        let created_at = created.value();
        let entry = MetadataCacheEntry::try_new(
            cache_key.clone(),
            vec![claim.claim_id()],
            created,
            created_at + Duration::hours(1),
            created_at + Duration::hours(2),
            created_at + Duration::days(2),
        )
        .expect("cache entry");
        write_metadata_cache_entry(
            &connection,
            node.access.workspace_id(),
            &entry,
            capability,
            correlation_id,
        )
        .expect("write cache");
        let loaded = load_metadata_cache_entry(
            &connection,
            node.access.workspace_id(),
            &cache_key,
            capability,
            correlation_id,
        )
        .expect("load cache")
        .expect("entry");
        assert_eq!(loaded, entry);
        assert_eq!(
            loaded.read_state(
                created_at + Duration::hours(3),
                MetadataCachePurpose::OfflineRead,
                MetadataDataClassification::Internal,
                true,
            ),
            MetadataCacheReadState::StaleOnError
        );

        let invalidated = entry
            .invalidated(
                MetadataCacheInvalidationReason::ProviderConfigurationChanged,
                received(400),
            )
            .expect("invalidate");
        write_metadata_cache_entry(
            &connection,
            node.access.workspace_id(),
            &invalidated,
            capability,
            correlation_id,
        )
        .expect("persist invalidation");
        assert_eq!(
            load_metadata_cache_entry(
                &connection,
                node.access.workspace_id(),
                &cache_key,
                capability,
                correlation_id,
            )
            .expect("load invalidated cache"),
            Some(invalidated)
        );

        assert_eq!(
            clear_metadata_cache_partition(
                &connection,
                node.access.workspace_id(),
                Some(cache_key.provider_id()),
                capability,
                correlation_id,
            )
            .expect("clear cache partition"),
            1
        );
        assert_eq!(
            load_metadata_cache_entry(
                &connection,
                node.access.workspace_id(),
                &cache_key,
                capability,
                correlation_id,
            )
            .expect("cache miss"),
            None
        );
        let claim_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM metadata_claims", [], |row| row.get(0))
            .expect("claim count");
        assert_eq!(claim_count, 1, "cache clearing must not delete claims");
        let columns = connection
            .prepare("PRAGMA table_info(metadata_cache_entries)")
            .expect("cache columns")
            .query_map([], |row| row.get::<_, String>(1))
            .expect("cache column rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("cache column names");
        assert!(!columns.iter().any(|column| column == "payload"));
    }

    #[test]
    fn enrichment_policy_and_attribution_round_trip() {
        let node = TestNode::new();
        let projection = MetadataProjectionPolicy::new(
            node.access.profile_id(),
            Some(MetadataProviderId::try_new("tmdb").expect("provider")),
            Some(MetadataLocale::try_new("en-IE").expect("locale")),
            None,
            true,
            LastKnownGoodPolicy::Allow,
        );
        let policy = EnrichmentPolicy::new(
            projection,
            Some(MetadataRegion::try_new("IE").expect("region")),
            vec![
                MetadataFieldGroup::BasicInfo,
                MetadataFieldGroup::Artwork,
                MetadataFieldGroup::BasicInfo,
            ],
        );
        let attribution = MetadataAttribution::try_new(
            MetadataProviderId::try_new("tmdb").expect("provider"),
            "Metadata supplied by TMDB",
            "https://www.themoviedb.org/documentation/api",
        )
        .expect("attribution");
        let connection = node.kernel.inner.connection.lock().expect("connection");
        write_enrichment_policy(
            &connection,
            node.access.workspace_id(),
            &policy,
            CapabilityKey::ListRecords,
            RequestCorrelationId::new_v7(),
        )
        .expect("policy");
        write_metadata_attribution(
            &connection,
            node.access.workspace_id(),
            &attribution,
            CapabilityKey::ListRecords,
            RequestCorrelationId::new_v7(),
        )
        .expect("attribution");
        assert_eq!(
            load_enrichment_policy(
                &connection,
                node.access.workspace_id(),
                node.access.profile_id(),
                CapabilityKey::ListRecords,
                RequestCorrelationId::new_v7(),
            )
            .expect("load policy"),
            policy
        );
        assert_eq!(
            load_metadata_attribution(
                &connection,
                node.access.workspace_id(),
                attribution.provider_id(),
                CapabilityKey::ListRecords,
                RequestCorrelationId::new_v7(),
            )
            .expect("load attribution"),
            Some(attribution)
        );
    }

    #[test]
    fn projection_port_returns_policy_when_record_has_no_selectable_field() {
        let node = TestNode::new();
        let record_id = a_record(&node);
        let view = node
            .kernel
            .authorize_and_read_projection(ReadMetadataProjectionQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record_id,
                false,
            ))
            .expect("empty projection retains editable profile policy");
        assert!(view.fields().is_empty());
        assert!(view.ratings().is_empty());
        assert_eq!(
            view.enrichment_policy().profile_id(),
            node.access.profile_id()
        );
    }

    #[test]
    fn projection_port_returns_last_known_good_with_exact_attribution() {
        let node = TestNode::new();
        let record_id = a_record(&node);
        let key = field_key("core.title");
        let claim = FieldClaim::try_new_provider(
            MetadataClaimId::new_v7(),
            record_id,
            key.clone(),
            "Expired but retained",
            provenance("movie-lkg"),
            received(100),
            Some(received(200).value()),
            FieldClaimStatus::Fresh,
        )
        .expect("expired provider claim");
        let unrelated = MetadataAttribution::try_new(
            MetadataProviderId::try_new("other").expect("provider"),
            "Unrelated provider",
            "https://example.com/docs",
        )
        .expect("unrelated attribution");
        let expected = MetadataAttribution::try_new(
            MetadataProviderId::try_new("tmdb").expect("provider"),
            "TMDB attribution",
            "https://www.themoviedb.org/documentation/api",
        )
        .expect("attribution");
        node.kernel
            .authorize_and_configure_projection(ConfigureMetadataProjectionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                MetadataProjectionPolicy::default_for_profile(node.access.profile_id()),
                None,
                vec![MetadataFieldGroup::BasicInfo],
                Vec::new(),
            ))
            .expect("enable basic metadata fields");
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            write_field_claim(
                &connection,
                node.access.workspace_id(),
                record_id,
                &key,
                &claim,
                CapabilityKey::ListRecords,
                RequestCorrelationId::new_v7(),
            )
            .expect("claim");
            for attribution in [&expected, &unrelated] {
                write_metadata_attribution(
                    &connection,
                    node.access.workspace_id(),
                    attribution,
                    CapabilityKey::ListRecords,
                    RequestCorrelationId::new_v7(),
                )
                .expect("attribution");
            }
        }
        let view = node
            .kernel
            .authorize_and_read_projection(ReadMetadataProjectionQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record_id,
                true,
            ))
            .expect("offline projection");
        assert_eq!(view.fields().len(), 1);
        assert_eq!(
            view.fields()[0].resolved_field().tier(),
            FieldResolutionTier::LastKnownGood
        );
        assert_eq!(view.attributions(), &[expected]);
    }

    #[test]
    fn configuring_projection_invalidates_only_matching_display_partition() {
        use chrono::Duration;

        let node = TestNode::new();
        let record_id = a_record(&node);
        let key = field_key("core.title");
        let claim = FieldClaim::try_new_provider(
            MetadataClaimId::new_v7(),
            record_id,
            key.clone(),
            "Projected title",
            provenance("movie-config"),
            received(100),
            None,
            FieldClaimStatus::Fresh,
        )
        .expect("claim");
        let capability = CapabilityKey::ListRecords;
        let correlation_id = RequestCorrelationId::new_v7();
        let prior_policy = EnrichmentPolicy::new(
            MetadataProjectionPolicy::default_for_profile(node.access.profile_id()),
            None,
            Vec::new(),
        );
        let fingerprint = enrichment_policy_fingerprint(&prior_policy, capability, correlation_id)
            .expect("fingerprint");
        let entry_for = |purpose, fingerprint: Sha256Digest| {
            let cache_key = MetadataCacheKey::try_new(
                MetadataProviderId::try_new("tmdb").expect("provider"),
                None,
                record_id,
                "metadata/movie",
                Grain::Film,
                ns("tmdb"),
                "movie-config",
                None,
                None,
                MetadataFieldGroup::BasicInfo,
                fingerprint,
                digest("d"),
                1,
                purpose,
                "terms-v1",
                MetadataDataClassification::Internal,
            )
            .expect("cache key");
            let created = received(300);
            MetadataCacheEntry::try_new(
                cache_key,
                vec![claim.claim_id()],
                created,
                created.value() + Duration::hours(1),
                created.value() + Duration::hours(2),
                created.value() + Duration::days(2),
            )
            .expect("cache")
        };
        let matching = entry_for(MetadataCachePurpose::DisplayProjection, fingerprint);
        let unaffected = entry_for(MetadataCachePurpose::MetadataEnrichment, digest("e"));
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            write_field_claim(
                &connection,
                node.access.workspace_id(),
                record_id,
                &key,
                &claim,
                capability,
                correlation_id,
            )
            .expect("claim");
            for entry in [&matching, &unaffected] {
                write_metadata_cache_entry(
                    &connection,
                    node.access.workspace_id(),
                    entry,
                    capability,
                    correlation_id,
                )
                .expect("cache");
            }
        }
        let updated = MetadataProjectionPolicy::new(
            node.access.profile_id(),
            Some(MetadataProviderId::try_new("tmdb").expect("provider")),
            None,
            None,
            false,
            LastKnownGoodPolicy::Allow,
        );
        let outcome = node
            .kernel
            .authorize_and_configure_projection(ConfigureMetadataProjectionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                updated,
                None,
                vec![MetadataFieldGroup::BasicInfo],
                Vec::new(),
            ))
            .expect("configure");
        assert_eq!(outcome.invalidated_cache_entries(), 1);
        let connection = node.kernel.inner.connection.lock().expect("connection");
        assert!(load_metadata_cache_entry(
            &connection,
            node.access.workspace_id(),
            matching.key(),
            capability,
            correlation_id,
        )
        .expect("matching cache")
        .expect("matching entry")
        .invalidation()
        .is_some());
        assert!(load_metadata_cache_entry(
            &connection,
            node.access.workspace_id(),
            unaffected.key(),
            capability,
            correlation_id,
        )
        .expect("unaffected cache")
        .expect("unaffected entry")
        .invalidation()
        .is_none());
    }

    #[test]
    fn refresh_rejects_field_groups_disabled_by_the_profile_policy() {
        let node = TestNode::new();
        let (record_id, provider_id, prepared) = refresh_fixture(&node);
        assert_eq!(prepared.field_groups(), &[MetadataFieldGroup::BasicInfo]);

        let result = node
            .kernel
            .authorize_and_prepare_refresh(PrepareMetadataRefreshCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record_id,
                provider_id,
                vec![MetadataFieldGroup::Details],
            ));
        assert!(matches!(
            result,
            Err(problem) if problem.code() == ProblemCode::ValidationFailed
        ));
    }

    #[test]
    fn projection_cache_keys_are_isolated_by_exact_policy_fingerprint() {
        use chrono::Duration;

        let node = TestNode::new();
        let record_id = a_record(&node);
        let key = field_key(TITLE_FIELD_KEY);
        let claim = FieldClaim::try_new_provider(
            MetadataClaimId::new_v7(),
            record_id,
            key.clone(),
            "Partitioned title",
            provenance("movie-partition"),
            received(100),
            None,
            FieldClaimStatus::Fresh,
        )
        .expect("claim");
        let entry = |fingerprint: Sha256Digest, route: &str| {
            let key = MetadataCacheKey::try_new(
                MetadataProviderId::try_new("tmdb").expect("provider"),
                Some(1),
                record_id,
                route,
                Grain::Film,
                ns("tmdb"),
                "movie-partition",
                None,
                None,
                MetadataFieldGroup::BasicInfo,
                fingerprint,
                digest("c"),
                1,
                MetadataCachePurpose::DisplayProjection,
                "terms-v1",
                MetadataDataClassification::Internal,
            )
            .expect("cache key");
            let created = received(300);
            MetadataCacheEntry::try_new(
                key,
                vec![claim.claim_id()],
                created,
                created.value() + Duration::hours(1),
                created.value() + Duration::hours(2),
                created.value() + Duration::days(2),
            )
            .expect("cache entry")
        };
        let first = entry(digest("d"), "metadata/first");
        let second = entry(digest("e"), "metadata/second");
        let connection = node.kernel.inner.connection.lock().expect("connection");
        write_field_claim(
            &connection,
            node.access.workspace_id(),
            record_id,
            &key,
            &claim,
            CapabilityKey::ListRecords,
            RequestCorrelationId::new_v7(),
        )
        .expect("claim");
        for cache in [&first, &second] {
            write_metadata_cache_entry(
                &connection,
                node.access.workspace_id(),
                cache,
                CapabilityKey::ListRecords,
                RequestCorrelationId::new_v7(),
            )
            .expect("cache");
        }
        let loaded = load_record_cache_keys(
            &connection,
            node.access.workspace_id(),
            record_id,
            first.key().settings_fingerprint(),
            CapabilityKey::ReadMetadataProjection,
            RequestCorrelationId::new_v7(),
        )
        .expect("partitioned keys");
        assert_eq!(loaded, vec![first.key().clone()]);
    }

    #[test]
    fn cache_claim_reference_cannot_cross_record_boundaries() {
        use chrono::Duration;

        let node = TestNode::new();
        let claim_record = a_record(&node);
        let cache_record = a_record(&node);
        let key = field_key(TITLE_FIELD_KEY);
        let claim = FieldClaim::try_new_provider(
            MetadataClaimId::new_v7(),
            claim_record,
            key.clone(),
            "Wrong record",
            provenance("movie-cross-record"),
            received(100),
            None,
            FieldClaimStatus::Fresh,
        )
        .expect("claim");
        let connection = node.kernel.inner.connection.lock().expect("connection");
        write_field_claim(
            &connection,
            node.access.workspace_id(),
            claim_record,
            &key,
            &claim,
            CapabilityKey::ListRecords,
            RequestCorrelationId::new_v7(),
        )
        .expect("claim");
        let cache_key = MetadataCacheKey::try_new(
            MetadataProviderId::try_new("tmdb").expect("provider"),
            Some(1),
            cache_record,
            "metadata/movie",
            Grain::Film,
            ns("tmdb"),
            "movie-cross-record",
            None,
            None,
            MetadataFieldGroup::BasicInfo,
            digest("d"),
            digest("e"),
            1,
            MetadataCachePurpose::MetadataEnrichment,
            "terms-v1",
            MetadataDataClassification::Public,
        )
        .expect("cache key");
        let created = received(300);
        let entry = MetadataCacheEntry::try_new(
            cache_key,
            vec![claim.claim_id()],
            created,
            created.value() + Duration::hours(1),
            created.value() + Duration::hours(2),
            created.value() + Duration::days(2),
        )
        .expect("cache entry");
        assert!(write_metadata_cache_entry(
            &connection,
            node.access.workspace_id(),
            &entry,
            CapabilityKey::RefreshMetadataClaims,
            RequestCorrelationId::new_v7(),
        )
        .is_err());
    }

    #[test]
    fn refresh_commit_rejects_a_changed_provider_state_snapshot() {
        let node = TestNode::new();
        let (record_id, provider_id, prepared) = refresh_fixture(&node);
        let expected = provider_state(1);
        node.kernel
            .put_provider_capability_state(node.access.workspace_id(), expected.clone())
            .expect("initial provider state");
        node.kernel
            .put_provider_capability_state(node.access.workspace_id(), provider_state(2))
            .expect("rotated provider state");
        let claim_id = MetadataClaimId::new_v7();
        let field = ProviderMetadataField::new(
            field_key(TITLE_FIELD_KEY),
            FieldClaim::try_new_provider(
                claim_id,
                record_id,
                field_key(TITLE_FIELD_KEY),
                "Never committed",
                FieldClaimProvenance::try_new(
                    provider_id.clone(),
                    ns("tmdb.movie"),
                    "438631",
                    Some(MetadataLocale::try_new("en-US").expect("locale")),
                    None,
                    Some("v3".to_owned()),
                    digest("a"),
                )
                .expect("provenance"),
                received(100),
                None,
                FieldClaimStatus::Fresh,
            )
            .expect("field claim"),
        );
        let attribution = MetadataAttribution::try_new(
            provider_id.clone(),
            "Metadata supplied by TMDB",
            "https://developer.themoviedb.org/",
        )
        .expect("attribution");
        let result = node
            .kernel
            .authorize_and_commit_refresh(CommitMetadataRefreshCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                prepared,
                provider_id,
                expected,
                vec![field],
                Vec::new(),
                Vec::new(),
                attribution,
            ));
        assert!(matches!(
            result,
            Err(problem) if problem.code() == ProblemCode::IntegrityFailed
        ));
        let connection = node.kernel.inner.connection.lock().expect("connection");
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM metadata_claims WHERE claim_id = ?1",
                [claim_id.to_string()],
                |row| row.get(0),
            )
            .expect("claim count");
        assert_eq!(count, 0);
    }

    #[test]
    fn provider_unavailability_appends_lifecycle_and_retains_last_known_good() {
        let node = TestNode::new();
        let (record_id, provider_id, prepared) = refresh_fixture(&node);
        let key = field_key(TITLE_FIELD_KEY);
        let claim = FieldClaim::try_new_provider(
            MetadataClaimId::new_v7(),
            record_id,
            key.clone(),
            "Retained title",
            FieldClaimProvenance::try_new(
                provider_id.clone(),
                ns("tmdb.movie"),
                "438631",
                Some(MetadataLocale::try_new("en-US").expect("locale")),
                None,
                Some("v3".to_owned()),
                digest("a"),
            )
            .expect("provenance"),
            received(101),
            None,
            FieldClaimStatus::Fresh,
        )
        .expect("claim");
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            write_field_claim(
                &connection,
                node.access.workspace_id(),
                record_id,
                &key,
                &claim,
                CapabilityKey::RefreshMetadataClaims,
                RequestCorrelationId::new_v7(),
            )
            .expect("claim");
        }
        node.kernel
            .authorize_and_mark_refresh_unavailable(MarkMetadataRefreshUnavailableCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                prepared,
                provider_id,
            ))
            .expect("mark provider unavailable");
        let view = node
            .kernel
            .authorize_and_read_projection(ReadMetadataProjectionQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record_id,
                true,
            ))
            .expect("last-known-good projection");
        assert_eq!(view.fields().len(), 1);
        assert_eq!(
            view.fields()[0].resolved_field().tier(),
            FieldResolutionTier::LastKnownGood
        );
        let connection = node.kernel.inner.connection.lock().expect("connection");
        let status: String = connection
            .query_row(
                "SELECT status FROM metadata_claim_lifecycle_events WHERE claim_id = ?1 AND sequence = 1",
                [claim.claim_id().to_string()],
                |row| row.get(0),
            )
            .expect("lifecycle status");
        assert_eq!(status, "unavailable");
    }

    #[test]
    fn provider_unavailability_transitions_every_claim_beyond_one_page() {
        let node = TestNode::new();
        let (record_id, provider_id, prepared) = refresh_fixture(&node);
        let key = field_key(TITLE_FIELD_KEY);
        {
            let connection = node.kernel.inner.connection.lock().expect("connection");
            for index in 0..513 {
                let claim = FieldClaim::try_new_provider(
                    MetadataClaimId::new_v7(),
                    record_id,
                    key.clone(),
                    format!("Title {index}"),
                    FieldClaimProvenance::try_new(
                        provider_id.clone(),
                        ns("tmdb.movie"),
                        format!("movie-{index}"),
                        None,
                        None,
                        Some("v3".to_owned()),
                        digest("a"),
                    )
                    .expect("provenance"),
                    received(100 + index),
                    None,
                    FieldClaimStatus::Fresh,
                )
                .expect("claim");
                write_field_claim(
                    &connection,
                    node.access.workspace_id(),
                    record_id,
                    &key,
                    &claim,
                    CapabilityKey::RefreshMetadataClaims,
                    RequestCorrelationId::new_v7(),
                )
                .expect("write claim");
            }
        }
        node.kernel
            .authorize_and_mark_refresh_unavailable(MarkMetadataRefreshUnavailableCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                prepared,
                provider_id,
            ))
            .expect("mark every provider claim unavailable");
        let connection = node.kernel.inner.connection.lock().expect("connection");
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM metadata_claim_lifecycle_events WHERE workspace_id = ?1 AND status = 'unavailable'",
                [node.access.workspace_id().to_string()],
                |row| row.get(0),
            )
            .expect("unavailable lifecycle count");
        assert_eq!(count, 513);
    }
}
