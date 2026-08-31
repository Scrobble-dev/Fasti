use crate::identity::load_record_grain;
use crate::kernel::{
    authorize_application_transaction, map_sql, now, parse_timestamp, timestamp, SqliteKernel,
};
use fasti_application::{
    plan_purpose_identity_route_with_evidence,
    preview_anime_grouping_change_for_record_with_evidence, AnimeGroupingPolicyChange,
    AnimeGroupingPolicyImpact, AnimeGroupingPolicyScope, AnimeGroupingPolicySource,
    AnimeGroupingPolicyView, ApplicationResult, ApplyAnimeGroupingPolicyChangeCommand,
    ApplyAnimeGroupingPolicyChangeOutcome, AuthorizedApplicationAccess, CapabilityKey,
    FastiProblem, IdentityRouteEvidence, IdentityRoutingPort,
    PreviewAnimeGroupingPolicyChangeQuery, ProblemCode, ReadAnimeGroupingPolicyOutcome,
    ReadAnimeGroupingPolicyQuery, ResolveIdentityRouteOutcome, ResolveIdentityRouteQuery,
    MAX_IDENTITY_CLAIMS,
};
use fasti_domain::{
    AnimeGroupingPreference, ClientId, EvidenceId, ExternalIdentifier, ExternalIdentifierClaim,
    ExternalIdentifierId, Grain, IdentityAssertion, IdentityAssertionEvidence,
    IdentityAssertionEvidenceClass, IdentityAssertionId, IdentityAssertionLifecycleEvent,
    IdentityAssertionRelation, IdentityAssertionStatus, IdentityCoverageMode,
    IdentityCoverageSegment, IdentityEpisodeLink, IdentityEpisodeLinkKind, IdentityEvidenceMethod,
    IdentityNumberingSpace, IdentityOrdering, OperationId, ProfileId, ReceivedAt, RecordId,
    RequestCorrelationId, Sha256Digest, WorkspaceId,
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Deserialize;
use std::collections::HashMap;
use std::str::FromStr;

const MAX_IDENTITY_ASSERTIONS_PER_RECORD: i64 = 256;
const MAX_IDENTITY_LIFECYCLE_EVENTS_PER_ASSERTION: i64 = 64;
const POLICY_PREVIEW_BATCH_SIZE: i64 = 256;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredCoverageSegment {
    mode: IdentityCoverageMode,
    season: Option<u32>,
    numbering_space: IdentityNumberingSpace,
    ordering: IdentityOrdering,
    source_start: u32,
    source_end: u32,
    offset: i32,
    region: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredEpisodeLink {
    from: Vec<u32>,
    to: Vec<u32>,
    kind: IdentityEpisodeLinkKind,
    reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredAssertionEvidence {
    method: IdentityEvidenceMethod,
    observed_source: String,
    derivation_root: Option<String>,
    reviewer: Option<String>,
    observed_at: chrono::NaiveDate,
    evidence_id: Option<EvidenceId>,
}

struct StoredAssertionRow {
    assertion_id: String,
    source_external_identifier_id: String,
    source_namespace: String,
    source_grain: String,
    source_value: String,
    target_namespace: String,
    target_grain: String,
    target_value: String,
    relation: String,
    coverage_json: String,
    episode_links_json: String,
    evidence_class: String,
    evidence_json: String,
    id_source: String,
    source_version: Option<String>,
    authority: Option<String>,
    reasoning: Option<String>,
    initial_status: String,
    created_at: String,
}

#[derive(Debug, Clone, Copy)]
struct PolicyState {
    preference: AnimeGroupingPreference,
    source: AnimeGroupingPolicySource,
    revision: u64,
}

#[derive(Debug, Clone, Copy)]
struct ReceiptState {
    previous_preference: AnimeGroupingPreference,
    previous_source: AnimeGroupingPolicySource,
    result_preference: AnimeGroupingPreference,
    result_source: AnimeGroupingPolicySource,
    result_revision: u64,
    affected_records: u64,
    unresolved_routes: u64,
    possible_season_regroupings: u64,
}

fn integrity(capability: CapabilityKey, correlation_id: RequestCorrelationId) -> Box<FastiProblem> {
    Box::new(FastiProblem::integrity_failed(capability, correlation_id))
}

fn validation(
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Box<FastiProblem> {
    Box::new(FastiProblem::from_code(
        ProblemCode::ValidationFailed,
        capability,
        correlation_id,
    ))
}

fn idempotency_conflict(
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> Box<FastiProblem> {
    Box::new(FastiProblem::from_code(
        ProblemCode::IdempotencyConflict,
        capability,
        correlation_id,
    ))
}

fn preference(value: &str) -> Option<AnimeGroupingPreference> {
    AnimeGroupingPreference::ALL
        .iter()
        .copied()
        .find(|candidate| candidate.as_str() == value)
}

fn policy_source(value: &str) -> Option<AnimeGroupingPolicySource> {
    match value {
        "profile_default" => Some(AnimeGroupingPolicySource::ProfileDefault),
        "client_override" => Some(AnimeGroupingPolicySource::ClientOverride),
        _ => None,
    }
}

const fn policy_source_value(value: AnimeGroupingPolicySource) -> &'static str {
    match value {
        AnimeGroupingPolicySource::ProfileDefault => "profile_default",
        AnimeGroupingPolicySource::ClientOverride => "client_override",
    }
}

fn assertion_relation(value: &str) -> Option<IdentityAssertionRelation> {
    match value {
        "exact" => Some(IdentityAssertionRelation::Exact),
        "subset_of" => Some(IdentityAssertionRelation::SubsetOf),
        "superset_of" => Some(IdentityAssertionRelation::SupersetOf),
        "overlaps" => Some(IdentityAssertionRelation::Overlaps),
        "alternate_cut_of" => Some(IdentityAssertionRelation::AlternateCutOf),
        "related" => Some(IdentityAssertionRelation::Related),
        "not_same_as" => Some(IdentityAssertionRelation::NotSameAs),
        _ => None,
    }
}

fn assertion_evidence_class(value: &str) -> Option<IdentityAssertionEvidenceClass> {
    match value {
        "asserted" => Some(IdentityAssertionEvidenceClass::Asserted),
        "verified" => Some(IdentityAssertionEvidenceClass::Verified),
        "corroborated" => Some(IdentityAssertionEvidenceClass::Corroborated),
        "inferred" => Some(IdentityAssertionEvidenceClass::Inferred),
        "candidate" => Some(IdentityAssertionEvidenceClass::Candidate),
        "disputed" => Some(IdentityAssertionEvidenceClass::Disputed),
        _ => None,
    }
}

fn assertion_status(value: &str) -> Option<IdentityAssertionStatus> {
    match value {
        "candidate" => Some(IdentityAssertionStatus::Candidate),
        "accepted" => Some(IdentityAssertionStatus::Accepted),
        "disputed" => Some(IdentityAssertionStatus::Disputed),
        "rejected" => Some(IdentityAssertionStatus::Rejected),
        "revoked" => Some(IdentityAssertionStatus::Revoked),
        _ => None,
    }
}

fn parse_u64(
    value: i64,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<u64> {
    u64::try_from(value).map_err(|_| integrity(capability, correlation_id))
}

fn load_policy_state(
    connection: &Connection,
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    scope: AnimeGroupingPolicyScope,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<PolicyState> {
    let profile = map_sql(
        connection
            .query_row(
                r#"
                SELECT preference, revision
                FROM profile_anime_grouping_policies
                WHERE workspace_id = ?1 AND profile_id = ?2
                "#,
                params![workspace_id.to_string(), profile_id.to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    let profile = match profile {
        Some((value, revision)) => PolicyState {
            preference: preference(&value).ok_or_else(|| integrity(capability, correlation_id))?,
            source: AnimeGroupingPolicySource::ProfileDefault,
            revision: parse_u64(revision, capability, correlation_id)?,
        },
        None => PolicyState {
            preference: AnimeGroupingPreference::Automatic,
            source: AnimeGroupingPolicySource::ProfileDefault,
            revision: 0,
        },
    };

    let AnimeGroupingPolicyScope::Client(client_id) = scope else {
        return Ok(profile);
    };
    let owns_profile: bool = map_sql(
        connection.query_row(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM profile_grants
                WHERE workspace_id = ?1 AND profile_id = ?2 AND client_id = ?3
                  AND status = 'active'
            )
            "#,
            params![
                workspace_id.to_string(),
                profile_id.to_string(),
                client_id.to_string()
            ],
            |row| row.get(0),
        ),
        capability,
        correlation_id,
    )?;
    if !owns_profile {
        return Err(validation(capability, correlation_id));
    }
    let client = map_sql(
        connection
            .query_row(
                r#"
                SELECT preference, revision
                FROM client_anime_grouping_policies
                WHERE workspace_id = ?1 AND profile_id = ?2 AND client_id = ?3
                "#,
                params![
                    workspace_id.to_string(),
                    profile_id.to_string(),
                    client_id.to_string()
                ],
                |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    match client {
        Some((Some(value), revision)) => Ok(PolicyState {
            preference: preference(&value).ok_or_else(|| integrity(capability, correlation_id))?,
            source: AnimeGroupingPolicySource::ClientOverride,
            revision: parse_u64(revision, capability, correlation_id)?,
        }),
        Some((None, revision)) => Ok(PolicyState {
            revision: profile
                .revision
                .max(parse_u64(revision, capability, correlation_id)?),
            ..profile
        }),
        None => Ok(profile),
    }
}

fn policy_view(
    profile_id: ProfileId,
    scope: AnimeGroupingPolicyScope,
    state: PolicyState,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<AnimeGroupingPolicyView> {
    AnimeGroupingPolicyView::try_new(
        profile_id,
        scope,
        state.source,
        state.preference,
        state.revision,
    )
    .map_err(|_| integrity(capability, correlation_id))
}

fn authorize_policy_scope(
    authorized: AuthorizedApplicationAccess,
    scope: AnimeGroupingPolicyScope,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    if matches!(scope, AnimeGroupingPolicyScope::Client(client_id) if client_id != authorized.attribution_client_id())
    {
        return Err(Box::new(FastiProblem::forbidden(
            capability,
            correlation_id,
        )));
    }
    Ok(())
}

fn load_lifecycle_events(
    connection: &Connection,
    workspace_id: WorkspaceId,
    assertion_id: IdentityAssertionId,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<Vec<IdentityAssertionLifecycleEvent>> {
    let mut statement = map_sql(
        connection.prepare(
            r#"
            SELECT sequence, previous_status, status, reviewer_client_id,
                   occurred_at, evidence_digest
            FROM identity_assertion_lifecycle_events
            WHERE workspace_id = ?1 AND assertion_id = ?2
            ORDER BY sequence
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
                assertion_id.to_string(),
                MAX_IDENTITY_LIFECYCLE_EVENTS_PER_ASSERTION + 1
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        ),
        capability,
        correlation_id,
    )?;
    let mut events = Vec::new();
    for row in rows {
        let (sequence, previous, status, reviewer, occurred_at, evidence_digest) =
            map_sql(row, capability, correlation_id)?;
        events.push(
            IdentityAssertionLifecycleEvent::try_new(
                assertion_id,
                u32::try_from(sequence).map_err(|_| integrity(capability, correlation_id))?,
                assertion_status(&previous).ok_or_else(|| integrity(capability, correlation_id))?,
                assertion_status(&status).ok_or_else(|| integrity(capability, correlation_id))?,
                ClientId::from_str(&reviewer).map_err(|_| integrity(capability, correlation_id))?,
                ReceivedAt::from_application_clock(parse_timestamp(
                    &occurred_at,
                    capability,
                    correlation_id,
                )?),
                evidence_digest
                    .map(Sha256Digest::parse)
                    .transpose()
                    .map_err(|_| integrity(capability, correlation_id))?,
            )
            .map_err(|_| integrity(capability, correlation_id))?,
        );
    }
    if events.len() > MAX_IDENTITY_LIFECYCLE_EVENTS_PER_ASSERTION as usize {
        return Err(Box::new(FastiProblem::capacity_exceeded(
            capability,
            correlation_id,
        )));
    }
    Ok(events)
}

fn materialize_assertion(
    row: StoredAssertionRow,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<IdentityAssertion> {
    let source_claim = ExternalIdentifierClaim::try_new(
        row.source_namespace,
        Grain::from_str(&row.source_grain).map_err(|_| integrity(capability, correlation_id))?,
        row.source_value,
    )
    .map_err(|_| integrity(capability, correlation_id))?;
    let source = ExternalIdentifier::new(
        ExternalIdentifierId::from_str(&row.source_external_identifier_id)
            .map_err(|_| integrity(capability, correlation_id))?,
        workspace_id,
        record_id,
        source_claim,
    );
    let target = ExternalIdentifierClaim::try_new(
        row.target_namespace,
        Grain::from_str(&row.target_grain).map_err(|_| integrity(capability, correlation_id))?,
        row.target_value,
    )
    .map_err(|_| integrity(capability, correlation_id))?;
    let coverage = serde_json::from_str::<Vec<StoredCoverageSegment>>(&row.coverage_json)
        .map_err(|_| integrity(capability, correlation_id))?
        .into_iter()
        .map(|value| {
            IdentityCoverageSegment::try_new(
                value.mode,
                value.season,
                value.numbering_space,
                value.ordering,
                value.source_start,
                value.source_end,
                value.offset,
                value.region,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| integrity(capability, correlation_id))?;
    let episode_links = serde_json::from_str::<Vec<StoredEpisodeLink>>(&row.episode_links_json)
        .map_err(|_| integrity(capability, correlation_id))?
        .into_iter()
        .map(|value| IdentityEpisodeLink::try_new(value.from, value.to, value.kind, value.reason))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| integrity(capability, correlation_id))?;
    let evidence = serde_json::from_str::<Vec<StoredAssertionEvidence>>(&row.evidence_json)
        .map_err(|_| integrity(capability, correlation_id))?
        .into_iter()
        .map(|value| {
            IdentityAssertionEvidence::try_new(
                value.method,
                value.observed_source,
                value.derivation_root,
                value.reviewer,
                value.observed_at,
                value.evidence_id,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| integrity(capability, correlation_id))?;
    IdentityAssertion::try_new(
        IdentityAssertionId::from_str(&row.assertion_id)
            .map_err(|_| integrity(capability, correlation_id))?,
        &source,
        target,
        assertion_relation(&row.relation).ok_or_else(|| integrity(capability, correlation_id))?,
        coverage,
        episode_links,
        assertion_evidence_class(&row.evidence_class)
            .ok_or_else(|| integrity(capability, correlation_id))?,
        evidence,
        row.id_source,
        row.source_version,
        row.authority,
        row.reasoning,
        assertion_status(&row.initial_status)
            .ok_or_else(|| integrity(capability, correlation_id))?,
        ReceivedAt::from_application_clock(parse_timestamp(
            &row.created_at,
            capability,
            correlation_id,
        )?),
    )
    .map_err(|_| integrity(capability, correlation_id))
}

fn load_route_evidence(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_id: RecordId,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<Vec<IdentityRouteEvidence>> {
    load_record_grain(
        connection,
        workspace_id,
        record_id,
        capability,
        correlation_id,
    )?;
    let mut evidence = Vec::new();
    let mut identifiers = map_sql(
        connection.prepare(
            r#"
            SELECT namespace, grain, value
            FROM external_identifiers
            WHERE workspace_id = ?1 AND record_id = ?2
            ORDER BY namespace, grain, value
            LIMIT ?3
            "#,
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        identifiers.query_map(
            params![
                workspace_id.to_string(),
                record_id.to_string(),
                i64::try_from(MAX_IDENTITY_CLAIMS).unwrap_or(i64::MAX) + 1
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        ),
        capability,
        correlation_id,
    )?;
    for row in rows {
        let (namespace, grain, value) = map_sql(row, capability, correlation_id)?;
        evidence.push(IdentityRouteEvidence::direct(
            ExternalIdentifierClaim::try_new(
                namespace,
                Grain::from_str(&grain).map_err(|_| integrity(capability, correlation_id))?,
                value,
            )
            .map_err(|_| integrity(capability, correlation_id))?,
        ));
    }
    drop(identifiers);
    if evidence.len() > MAX_IDENTITY_CLAIMS {
        return Err(Box::new(FastiProblem::capacity_exceeded(
            capability,
            correlation_id,
        )));
    }

    let mut assertions = map_sql(
        connection.prepare(
            r#"
            SELECT assertion.assertion_id, assertion.source_external_identifier_id,
                   source.namespace, source.grain, source.value,
                   assertion.target_namespace, assertion.target_grain,
                   assertion.target_value, assertion.relation,
                   assertion.coverage_json, assertion.episode_links_json,
                   assertion.evidence_class, assertion.evidence_json,
                   assertion.id_source, assertion.source_version,
                   assertion.authority, assertion.reasoning,
                   assertion.initial_status, assertion.created_at
            FROM identity_assertions assertion
            JOIN external_identifiers source
              ON source.external_identifier_id = assertion.source_external_identifier_id
             AND source.workspace_id = assertion.workspace_id
             AND source.record_id = assertion.record_id
            WHERE assertion.workspace_id = ?1 AND assertion.record_id = ?2
            ORDER BY assertion.assertion_id
            LIMIT ?3
            "#,
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        assertions.query_map(
            params![
                workspace_id.to_string(),
                record_id.to_string(),
                MAX_IDENTITY_ASSERTIONS_PER_RECORD + 1
            ],
            |row| {
                Ok(StoredAssertionRow {
                    assertion_id: row.get(0)?,
                    source_external_identifier_id: row.get(1)?,
                    source_namespace: row.get(2)?,
                    source_grain: row.get(3)?,
                    source_value: row.get(4)?,
                    target_namespace: row.get(5)?,
                    target_grain: row.get(6)?,
                    target_value: row.get(7)?,
                    relation: row.get(8)?,
                    coverage_json: row.get(9)?,
                    episode_links_json: row.get(10)?,
                    evidence_class: row.get(11)?,
                    evidence_json: row.get(12)?,
                    id_source: row.get(13)?,
                    source_version: row.get(14)?,
                    authority: row.get(15)?,
                    reasoning: row.get(16)?,
                    initial_status: row.get(17)?,
                    created_at: row.get(18)?,
                })
            },
        ),
        capability,
        correlation_id,
    )?;
    let mut assertion_count = 0_usize;
    for row in rows {
        assertion_count += 1;
        if assertion_count > MAX_IDENTITY_ASSERTIONS_PER_RECORD as usize {
            return Err(Box::new(FastiProblem::capacity_exceeded(
                capability,
                correlation_id,
            )));
        }
        let assertion = materialize_assertion(
            map_sql(row, capability, correlation_id)?,
            workspace_id,
            record_id,
            capability,
            correlation_id,
        )?;
        let lifecycle = load_lifecycle_events(
            connection,
            workspace_id,
            assertion.assertion_id(),
            capability,
            correlation_id,
        )?;
        assertion
            .effective_status(&lifecycle)
            .map_err(|_| integrity(capability, correlation_id))?;
        if let Some(route) = IdentityRouteEvidence::accepted_crosswalk(&assertion, &lifecycle) {
            evidence.push(route);
        }
    }
    Ok(evidence)
}

pub(crate) fn validate_workspace_identity_routing_state(
    connection: &Connection,
    workspace_id: WorkspaceId,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    let capability = CapabilityKey::RestoreWorkspace;
    let mut statement = map_sql(
        connection.prepare(
            "SELECT DISTINCT record_id FROM identity_assertions WHERE workspace_id = ?1 ORDER BY record_id",
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map([workspace_id.to_string()], |row| row.get::<_, String>(0)),
        capability,
        correlation_id,
    )?;
    let record_ids = rows
        .map(|row| {
            map_sql(row, capability, correlation_id).and_then(|value| {
                RecordId::from_str(&value).map_err(|_| integrity(capability, correlation_id))
            })
        })
        .collect::<ApplicationResult<Vec<_>>>()?;
    drop(statement);
    for record_id in record_ids {
        load_route_evidence(
            connection,
            workspace_id,
            record_id,
            capability,
            correlation_id,
        )?;
    }
    Ok(())
}

fn read_rollback_state(
    connection: &Connection,
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    scope: AnimeGroupingPolicyScope,
    operation_id: OperationId,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<(AnimeGroupingPreference, AnimeGroupingPolicySource)> {
    let (scope_kind, scope_client_id) = match scope {
        AnimeGroupingPolicyScope::Profile => ("profile", None),
        AnimeGroupingPolicyScope::Client(client_id) => ("client", Some(client_id.to_string())),
    };
    let row = map_sql(
        connection
            .query_row(
                r#"
                SELECT previous_preference, previous_source
                FROM anime_grouping_policy_receipts
                WHERE workspace_id = ?1 AND profile_id = ?2
                  AND scope_kind = ?3 AND scope_client_id IS ?4
                  AND operation_id = ?5
                "#,
                params![
                    workspace_id.to_string(),
                    profile_id.to_string(),
                    scope_kind,
                    scope_client_id,
                    operation_id.to_string()
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional(),
        capability,
        correlation_id,
    )?
    .ok_or_else(|| validation(capability, correlation_id))?;
    Ok((
        preference(&row.0).ok_or_else(|| integrity(capability, correlation_id))?,
        policy_source(&row.1).ok_or_else(|| integrity(capability, correlation_id))?,
    ))
}

fn proposed_state(
    connection: &Connection,
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    scope: AnimeGroupingPolicyScope,
    change: AnimeGroupingPolicyChange,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<(AnimeGroupingPreference, AnimeGroupingPolicySource)> {
    match change {
        AnimeGroupingPolicyChange::Set(preference) => Ok((
            preference,
            match scope {
                AnimeGroupingPolicyScope::Profile => AnimeGroupingPolicySource::ProfileDefault,
                AnimeGroupingPolicyScope::Client(_) => AnimeGroupingPolicySource::ClientOverride,
            },
        )),
        AnimeGroupingPolicyChange::InheritProfile => {
            let profile = load_policy_state(
                connection,
                workspace_id,
                profile_id,
                AnimeGroupingPolicyScope::Profile,
                capability,
                correlation_id,
            )?;
            Ok((
                profile.preference,
                AnimeGroupingPolicySource::ProfileDefault,
            ))
        }
        AnimeGroupingPolicyChange::Rollback {
            applied_operation_id,
        } => read_rollback_state(
            connection,
            workspace_id,
            profile_id,
            scope,
            applied_operation_id,
            capability,
            correlation_id,
        ),
    }
}

fn load_preview_route_evidence(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_ids: &[RecordId],
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<HashMap<RecordId, Vec<IdentityRouteEvidence>>> {
    let Some(first) = record_ids.first() else {
        return Ok(HashMap::new());
    };
    let last = record_ids.last().expect("non-empty record batch");
    let mut evidence = record_ids
        .iter()
        .copied()
        .map(|record_id| (record_id, Vec::new()))
        .collect::<HashMap<_, _>>();

    let mut identifiers = map_sql(
        connection.prepare(
            r#"
            SELECT identifier.record_id, identifier.namespace,
                   identifier.grain, identifier.value
            FROM external_identifiers identifier
            JOIN records record
              ON record.workspace_id = identifier.workspace_id
             AND record.record_id = identifier.record_id
             AND record.status = 'active'
            WHERE identifier.workspace_id = ?1
              AND identifier.record_id >= ?2
              AND identifier.record_id <= ?3
            ORDER BY identifier.record_id, identifier.namespace,
                     identifier.grain, identifier.value
            LIMIT ?4
            "#,
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        identifiers.query_map(
            params![
                workspace_id.to_string(),
                first.to_string(),
                last.to_string(),
                i64::try_from(record_ids.len() * (MAX_IDENTITY_CLAIMS + 1)).unwrap_or(i64::MAX) + 1
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
    let mut identifier_rows = 0_usize;
    for row in rows {
        identifier_rows += 1;
        if identifier_rows > record_ids.len() * (MAX_IDENTITY_CLAIMS + 1) {
            return Err(Box::new(FastiProblem::capacity_exceeded(
                capability,
                correlation_id,
            )));
        }
        let (record_id, namespace, grain, value) = map_sql(row, capability, correlation_id)?;
        let record_id =
            RecordId::from_str(&record_id).map_err(|_| integrity(capability, correlation_id))?;
        let record_evidence = evidence
            .get_mut(&record_id)
            .ok_or_else(|| integrity(capability, correlation_id))?;
        if record_evidence.len() >= MAX_IDENTITY_CLAIMS {
            return Err(Box::new(FastiProblem::capacity_exceeded(
                capability,
                correlation_id,
            )));
        }
        record_evidence.push(IdentityRouteEvidence::direct(
            ExternalIdentifierClaim::try_new(
                namespace,
                Grain::from_str(&grain).map_err(|_| integrity(capability, correlation_id))?,
                value,
            )
            .map_err(|_| integrity(capability, correlation_id))?,
        ));
    }
    drop(identifiers);

    let mut assertions = map_sql(
        connection.prepare(
            r#"
            SELECT assertion.record_id, assertion.assertion_id,
                   assertion.source_external_identifier_id,
                   source.namespace, source.grain, source.value,
                   assertion.target_namespace, assertion.target_grain,
                   assertion.target_value, assertion.relation,
                   assertion.coverage_json, assertion.episode_links_json,
                   assertion.evidence_class, assertion.evidence_json,
                   assertion.id_source, assertion.source_version,
                   assertion.authority, assertion.reasoning,
                   assertion.initial_status, assertion.created_at
            FROM identity_assertions assertion
            JOIN external_identifiers source
              ON source.external_identifier_id = assertion.source_external_identifier_id
             AND source.workspace_id = assertion.workspace_id
             AND source.record_id = assertion.record_id
            JOIN records record
              ON record.workspace_id = assertion.workspace_id
             AND record.record_id = assertion.record_id
             AND record.status = 'active'
            WHERE assertion.workspace_id = ?1
              AND assertion.record_id >= ?2
              AND assertion.record_id <= ?3
            ORDER BY assertion.record_id, assertion.assertion_id
            LIMIT ?4
            "#,
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        assertions.query_map(
            params![
                workspace_id.to_string(),
                first.to_string(),
                last.to_string(),
                i64::try_from(record_ids.len() * (MAX_IDENTITY_ASSERTIONS_PER_RECORD as usize + 1))
                    .unwrap_or(i64::MAX)
                    + 1
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    StoredAssertionRow {
                        assertion_id: row.get(1)?,
                        source_external_identifier_id: row.get(2)?,
                        source_namespace: row.get(3)?,
                        source_grain: row.get(4)?,
                        source_value: row.get(5)?,
                        target_namespace: row.get(6)?,
                        target_grain: row.get(7)?,
                        target_value: row.get(8)?,
                        relation: row.get(9)?,
                        coverage_json: row.get(10)?,
                        episode_links_json: row.get(11)?,
                        evidence_class: row.get(12)?,
                        evidence_json: row.get(13)?,
                        id_source: row.get(14)?,
                        source_version: row.get(15)?,
                        authority: row.get(16)?,
                        reasoning: row.get(17)?,
                        initial_status: row.get(18)?,
                        created_at: row.get(19)?,
                    },
                ))
            },
        ),
        capability,
        correlation_id,
    )?;
    let mut assertion_counts = HashMap::<RecordId, usize>::new();
    for row in rows {
        let (record_id, row) = map_sql(row, capability, correlation_id)?;
        let record_id =
            RecordId::from_str(&record_id).map_err(|_| integrity(capability, correlation_id))?;
        let count = assertion_counts.entry(record_id).or_default();
        *count += 1;
        if *count > MAX_IDENTITY_ASSERTIONS_PER_RECORD as usize {
            return Err(Box::new(FastiProblem::capacity_exceeded(
                capability,
                correlation_id,
            )));
        }
        let assertion =
            materialize_assertion(row, workspace_id, record_id, capability, correlation_id)?;
        let lifecycle = load_lifecycle_events(
            connection,
            workspace_id,
            assertion.assertion_id(),
            capability,
            correlation_id,
        )?;
        assertion
            .effective_status(&lifecycle)
            .map_err(|_| integrity(capability, correlation_id))?;
        if let Some(route) = IdentityRouteEvidence::accepted_crosswalk(&assertion, &lifecycle) {
            evidence
                .get_mut(&record_id)
                .ok_or_else(|| integrity(capability, correlation_id))?
                .push(route);
        }
    }
    Ok(evidence)
}

#[allow(clippy::too_many_arguments)]
fn calculate_policy_impact(
    connection: &Connection,
    workspace_id: WorkspaceId,
    profile_id: ProfileId,
    scope: AnimeGroupingPolicyScope,
    current: PolicyState,
    proposed_preference: AnimeGroupingPreference,
    proposed_source: AnimeGroupingPolicySource,
    query: &PreviewAnimeGroupingPolicyChangeQuery,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<AnimeGroupingPolicyImpact> {
    let mut total_records = 0_u64;
    let mut affected_records = 0_u64;
    let mut unresolved_routes = 0_u64;
    let mut possible_season_regroupings = 0_u64;
    let mut page = Vec::new();
    let mut has_more = false;
    let page_limit = usize::from(query.limit().get());
    let mut scan_after = String::new();
    loop {
        let mut statement = map_sql(
            connection.prepare(
                r#"
                SELECT record_id
                FROM records
                WHERE workspace_id = ?1 AND status = 'active' AND record_id > ?2
                ORDER BY record_id
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
                    scan_after,
                    POLICY_PREVIEW_BATCH_SIZE
                ],
                |row| row.get::<_, String>(0),
            ),
            capability,
            correlation_id,
        )?;
        let mut record_ids = Vec::new();
        for row in rows {
            record_ids.push(
                RecordId::from_str(&map_sql(row, capability, correlation_id)?)
                    .map_err(|_| integrity(capability, correlation_id))?,
            );
        }
        drop(statement);
        if record_ids.is_empty() {
            break;
        }
        scan_after = record_ids
            .last()
            .expect("non-empty record batch")
            .to_string();
        let mut evidence = load_preview_route_evidence(
            connection,
            workspace_id,
            &record_ids,
            capability,
            correlation_id,
        )?;
        for record_id in record_ids {
            total_records = total_records.checked_add(1).ok_or_else(|| {
                Box::new(FastiProblem::capacity_exceeded(capability, correlation_id))
            })?;
            let preview = preview_anime_grouping_change_for_record_with_evidence(
                record_id,
                current.preference,
                proposed_preference,
                evidence.remove(&record_id).as_deref().unwrap_or_default(),
            );
            affected_records += u64::from(preview.route_changed());
            unresolved_routes += u64::from(preview.unresolved());
            possible_season_regroupings += u64::from(preview.possible_season_regrouping());
            let after_cursor = query
                .after_record_id()
                .is_none_or(|cursor| record_id.uuid() > cursor.uuid());
            if after_cursor {
                if page.len() < page_limit {
                    page.push(preview);
                } else {
                    has_more = true;
                }
            }
        }
    }
    let next_after_record_id = has_more
        .then(|| page.last().map(|record| record.record_id()))
        .flatten();
    let current_view = policy_view(profile_id, scope, current, capability, correlation_id)?;
    AnimeGroupingPolicyImpact::try_new_authorized(
        profile_id,
        query,
        current_view,
        proposed_preference,
        proposed_source,
        total_records,
        affected_records,
        unresolved_routes,
        possible_season_regroupings,
        page,
        next_after_record_id,
    )
    .map_err(|_| integrity(capability, correlation_id))
}

fn receipt_scope(scope: AnimeGroupingPolicyScope) -> (&'static str, Option<String>) {
    match scope {
        AnimeGroupingPolicyScope::Profile => ("profile", None),
        AnimeGroupingPolicyScope::Client(client_id) => ("client", Some(client_id.to_string())),
    }
}

fn load_replay_receipt(
    connection: &Connection,
    workspace_id: WorkspaceId,
    actor_client_id: ClientId,
    command: &ApplyAnimeGroupingPolicyChangeCommand,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<Option<ReceiptState>> {
    let row = map_sql(
        connection
            .query_row(
                r#"
                SELECT semantic_digest, previous_preference, previous_source,
                       result_preference, result_source, result_revision,
                       affected_records, unresolved_routes,
                       possible_season_regroupings
                FROM anime_grouping_policy_receipts
                WHERE workspace_id = ?1 AND actor_client_id = ?2 AND operation_id = ?3
                "#,
                params![
                    workspace_id.to_string(),
                    actor_client_id.to_string(),
                    command.operation_id().to_string()
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, i64>(8)?,
                    ))
                },
            )
            .optional(),
        capability,
        correlation_id,
    )?;
    let Some(row) = row else {
        return Ok(None);
    };
    if row.0 != command.semantic_digest().as_str() {
        return Err(idempotency_conflict(capability, correlation_id));
    }
    Ok(Some(ReceiptState {
        previous_preference: preference(&row.1)
            .ok_or_else(|| integrity(capability, correlation_id))?,
        previous_source: policy_source(&row.2)
            .ok_or_else(|| integrity(capability, correlation_id))?,
        result_preference: preference(&row.3)
            .ok_or_else(|| integrity(capability, correlation_id))?,
        result_source: policy_source(&row.4)
            .ok_or_else(|| integrity(capability, correlation_id))?,
        result_revision: parse_u64(row.5, capability, correlation_id)?,
        affected_records: parse_u64(row.6, capability, correlation_id)?,
        unresolved_routes: parse_u64(row.7, capability, correlation_id)?,
        possible_season_regroupings: parse_u64(row.8, capability, correlation_id)?,
    }))
}

impl IdentityRoutingPort for SqliteKernel {
    fn authorize_and_resolve_identity(
        &self,
        query: ResolveIdentityRouteQuery,
    ) -> ApplicationResult<ResolveIdentityRouteOutcome> {
        let capability = CapabilityKey::ResolveIdentityRoute;
        let correlation_id = query.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Deferred),
            capability,
            correlation_id,
        )?;
        let authorized = authorize_application_transaction(
            &transaction,
            capability,
            query.access(),
            correlation_id,
        )?;
        let evidence = load_route_evidence(
            &transaction,
            authorized.workspace_id(),
            query.record_id(),
            capability,
            correlation_id,
        )?;
        let policy = load_policy_state(
            &transaction,
            authorized.workspace_id(),
            authorized.profile_id(),
            AnimeGroupingPolicyScope::Profile,
            capability,
            correlation_id,
        )?;
        let plan = plan_purpose_identity_route_with_evidence(
            query.record_id(),
            query.intent(),
            query.target_provider().clone(),
            policy.preference,
            &evidence,
        );
        let outcome = ResolveIdentityRouteOutcome::try_new(&query, plan)
            .map_err(|_| integrity(capability, correlation_id))?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(outcome)
    }

    fn authorize_and_read_anime_grouping_policy(
        &self,
        query: ReadAnimeGroupingPolicyQuery,
    ) -> ApplicationResult<ReadAnimeGroupingPolicyOutcome> {
        let capability = CapabilityKey::ReadAnimeGroupingPolicy;
        let correlation_id = query.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Deferred),
            capability,
            correlation_id,
        )?;
        let authorized = authorize_application_transaction(
            &transaction,
            capability,
            query.access(),
            correlation_id,
        )?;
        authorize_policy_scope(authorized, query.scope(), capability, correlation_id)?;
        let state = load_policy_state(
            &transaction,
            authorized.workspace_id(),
            authorized.profile_id(),
            query.scope(),
            capability,
            correlation_id,
        )?;
        let policy = policy_view(
            authorized.profile_id(),
            query.scope(),
            state,
            capability,
            correlation_id,
        )?;
        let outcome = ReadAnimeGroupingPolicyOutcome::try_new_authorized(
            authorized.profile_id(),
            &query,
            policy,
        )
        .map_err(|_| integrity(capability, correlation_id))?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(outcome)
    }

    fn authorize_and_preview_anime_grouping_policy_change(
        &self,
        query: PreviewAnimeGroupingPolicyChangeQuery,
    ) -> ApplicationResult<AnimeGroupingPolicyImpact> {
        let capability = CapabilityKey::PreviewAnimeGroupingPolicyChange;
        let correlation_id = query.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Deferred),
            capability,
            correlation_id,
        )?;
        let authorized = authorize_application_transaction(
            &transaction,
            capability,
            query.access(),
            correlation_id,
        )?;
        authorize_policy_scope(authorized, query.scope(), capability, correlation_id)?;
        let current = load_policy_state(
            &transaction,
            authorized.workspace_id(),
            authorized.profile_id(),
            query.scope(),
            capability,
            correlation_id,
        )?;
        let (proposed_preference, proposed_source) = proposed_state(
            &transaction,
            authorized.workspace_id(),
            authorized.profile_id(),
            query.scope(),
            query.change(),
            capability,
            correlation_id,
        )?;
        let outcome = calculate_policy_impact(
            &transaction,
            authorized.workspace_id(),
            authorized.profile_id(),
            query.scope(),
            current,
            proposed_preference,
            proposed_source,
            &query,
            capability,
            correlation_id,
        )?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(outcome)
    }

    fn authorize_and_apply_anime_grouping_policy_change(
        &self,
        command: ApplyAnimeGroupingPolicyChangeCommand,
    ) -> ApplicationResult<ApplyAnimeGroupingPolicyChangeOutcome> {
        let capability = CapabilityKey::ApplyAnimeGroupingPolicyChange;
        let correlation_id = command.correlation_id();
        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        let authorized = authorize_application_transaction(
            &transaction,
            capability,
            command.access(),
            correlation_id,
        )?;
        authorize_policy_scope(authorized, command.scope(), capability, correlation_id)?;
        let actor_client_id = authorized.attribution_client_id();
        if let Some(receipt) = load_replay_receipt(
            &transaction,
            authorized.workspace_id(),
            actor_client_id,
            &command,
            capability,
            correlation_id,
        )? {
            let result = policy_view(
                authorized.profile_id(),
                command.scope(),
                PolicyState {
                    preference: receipt.result_preference,
                    source: receipt.result_source,
                    revision: receipt.result_revision,
                },
                capability,
                correlation_id,
            )?;
            let outcome = ApplyAnimeGroupingPolicyChangeOutcome::try_new_authorized(
                authorized.profile_id(),
                &command,
                receipt.previous_preference,
                receipt.previous_source,
                result,
                receipt.affected_records,
                receipt.unresolved_routes,
                receipt.possible_season_regroupings,
            )
            .map_err(|_| integrity(capability, correlation_id))?;
            map_sql(transaction.commit(), capability, correlation_id)?;
            return Ok(outcome);
        }

        let current = load_policy_state(
            &transaction,
            authorized.workspace_id(),
            authorized.profile_id(),
            command.scope(),
            capability,
            correlation_id,
        )?;
        if current.revision != command.expected_revision() {
            return Err(idempotency_conflict(capability, correlation_id));
        }
        let (result_preference, result_source) = proposed_state(
            &transaction,
            authorized.workspace_id(),
            authorized.profile_id(),
            command.scope(),
            command.change(),
            capability,
            correlation_id,
        )?;
        let next_revision = current
            .revision
            .checked_add(1)
            .filter(|revision| *revision <= 9_007_199_254_740_991)
            .ok_or_else(|| Box::new(FastiProblem::capacity_exceeded(capability, correlation_id)))?;

        let preview_query = PreviewAnimeGroupingPolicyChangeQuery::try_new(
            correlation_id,
            command.access().clone(),
            command.scope(),
            command.change(),
            None,
            fasti_application::IdentityImpactPageLimit::try_new(1)
                .expect("one is a valid fixed preview page"),
        )
        .map_err(|_| validation(capability, correlation_id))?;
        let impact = calculate_policy_impact(
            &transaction,
            authorized.workspace_id(),
            authorized.profile_id(),
            command.scope(),
            current,
            result_preference,
            result_source,
            &preview_query,
            capability,
            correlation_id,
        )?;

        match command.scope() {
            AnimeGroupingPolicyScope::Profile => {
                map_sql(
                    transaction.execute(
                        r#"
                        INSERT INTO profile_anime_grouping_policies(
                            workspace_id, profile_id, preference, revision, updated_at
                        ) VALUES (?1, ?2, ?3, ?4, ?5)
                        ON CONFLICT(workspace_id, profile_id) DO UPDATE SET
                            preference = excluded.preference,
                            revision = excluded.revision,
                            updated_at = excluded.updated_at
                        WHERE profile_anime_grouping_policies.revision = ?6
                        "#,
                        params![
                            authorized.workspace_id().to_string(),
                            authorized.profile_id().to_string(),
                            result_preference.as_str(),
                            i64::try_from(next_revision).unwrap_or(i64::MAX),
                            timestamp(now()),
                            i64::try_from(current.revision).unwrap_or(i64::MAX)
                        ],
                    ),
                    capability,
                    correlation_id,
                )?;
            }
            AnimeGroupingPolicyScope::Client(client_id) => {
                let stored_preference = (result_source
                    == AnimeGroupingPolicySource::ClientOverride)
                    .then_some(result_preference.as_str());
                map_sql(
                    transaction.execute(
                        r#"
                        INSERT INTO client_anime_grouping_policies(
                            workspace_id, profile_id, client_id, preference, revision, updated_at
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                        ON CONFLICT(workspace_id, profile_id, client_id) DO UPDATE SET
                            preference = excluded.preference,
                            revision = excluded.revision,
                            updated_at = excluded.updated_at
                        "#,
                        params![
                            authorized.workspace_id().to_string(),
                            authorized.profile_id().to_string(),
                            client_id.to_string(),
                            stored_preference,
                            i64::try_from(next_revision).unwrap_or(i64::MAX),
                            timestamp(now())
                        ],
                    ),
                    capability,
                    correlation_id,
                )?;
            }
        }
        let (scope_kind, scope_client_id) = receipt_scope(command.scope());
        let (change_kind, requested_preference, rollback_operation_id) = match command.change() {
            AnimeGroupingPolicyChange::Set(preference) => ("set", Some(preference.as_str()), None),
            AnimeGroupingPolicyChange::InheritProfile => ("inherit_profile", None, None),
            AnimeGroupingPolicyChange::Rollback {
                applied_operation_id,
            } => ("rollback", None, Some(applied_operation_id.to_string())),
        };
        map_sql(
            transaction.execute(
                r#"
                INSERT INTO anime_grouping_policy_receipts(
                    workspace_id, profile_id, actor_client_id, scope_kind,
                    scope_client_id, operation_id, semantic_digest, change_kind,
                    requested_preference, rollback_operation_id,
                    previous_preference, previous_source, result_preference,
                    result_source, result_revision, affected_records,
                    unresolved_routes, possible_season_regroupings, created_at
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                    ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19
                )
                "#,
                params![
                    authorized.workspace_id().to_string(),
                    authorized.profile_id().to_string(),
                    actor_client_id.to_string(),
                    scope_kind,
                    scope_client_id,
                    command.operation_id().to_string(),
                    command.semantic_digest().as_str(),
                    change_kind,
                    requested_preference,
                    rollback_operation_id,
                    current.preference.as_str(),
                    policy_source_value(current.source),
                    result_preference.as_str(),
                    policy_source_value(result_source),
                    i64::try_from(next_revision).unwrap_or(i64::MAX),
                    i64::try_from(impact.affected_records()).unwrap_or(i64::MAX),
                    i64::try_from(impact.unresolved_routes()).unwrap_or(i64::MAX),
                    i64::try_from(impact.possible_season_regroupings()).unwrap_or(i64::MAX),
                    timestamp(now())
                ],
            ),
            capability,
            correlation_id,
        )?;
        let result = policy_view(
            authorized.profile_id(),
            command.scope(),
            PolicyState {
                preference: result_preference,
                source: result_source,
                revision: next_revision,
            },
            capability,
            correlation_id,
        )?;
        let outcome = ApplyAnimeGroupingPolicyChangeOutcome::try_new_authorized(
            authorized.profile_id(),
            &command,
            current.preference,
            current.source,
            result,
            impact.affected_records(),
            impact.unresolved_routes(),
            impact.possible_season_regroupings(),
        )
        .map_err(|_| integrity(capability, correlation_id))?;
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestNode;
    use fasti_application::{
        AttachIdentifierCommand, CreateRecordCommand, IdentityPort,
        RegisterNamespaceDefinitionCommand, ScopeKey,
    };
    use fasti_domain::{NamespaceDefinition, NamespaceLicencePosture};

    fn digest(byte: u8) -> Sha256Digest {
        Sha256Digest::parse(format!("sha256:{}", format!("{byte:02x}").repeat(32))).expect("digest")
    }

    fn create_anime_record(node: &TestNode) -> RecordId {
        for namespace in ["mal.anime", "imdb.title"] {
            node.kernel
                .register_namespace_definition(RegisterNamespaceDefinitionCommand::new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    NamespaceDefinition::try_new(
                        namespace,
                        namespace,
                        [Grain::Release],
                        ".+",
                        "identity",
                        NamespaceLicencePosture::IdentifiersOnly,
                    )
                    .expect("namespace definition"),
                ))
                .expect("register namespace");
        }
        let record_id = node
            .kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Release,
            ))
            .expect("create record")
            .record_id();
        node.kernel
            .attach_identifier(AttachIdentifierCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record_id,
                ExternalIdentifierClaim::try_new("mal.anime", Grain::Release, "49894")
                    .expect("MAL identifier"),
            ))
            .expect("attach MAL identifier");
        node.kernel
            .attach_identifier(AttachIdentifierCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record_id,
                ExternalIdentifierClaim::try_new("imdb.title", Grain::Release, "tt28254942")
                    .expect("IMDb identifier"),
            ))
            .expect("attach IMDb identifier");
        record_id
    }

    #[test]
    fn route_resolution_and_policy_change_are_durable_and_idempotent() {
        let node = TestNode::new();
        let record_id = create_anime_record(&node);
        let route = node
            .kernel
            .authorize_and_resolve_identity(ResolveIdentityRouteQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
                record_id,
                fasti_domain::ResolutionIntent::MetadataEnrichment,
                fasti_application::ProviderId::try_new("tmdb").expect("provider"),
            ))
            .expect("resolve route");
        assert_eq!(
            route
                .plan()
                .selected_route()
                .expect("selected alias")
                .identifier()
                .value(),
            "tt28254942"
        );

        let before = node
            .kernel
            .authorize_and_read_anime_grouping_policy(ReadAnimeGroupingPolicyQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
                AnimeGroupingPolicyScope::Profile,
            ))
            .expect("read default");
        assert_eq!(before.policy().revision(), 0);
        assert_eq!(
            before.policy().preference(),
            AnimeGroupingPreference::Automatic
        );

        let operation_id = OperationId::new_v7();
        let command = ApplyAnimeGroupingPolicyChangeCommand::try_new(
            RequestCorrelationId::new_v7(),
            node.access,
            AnimeGroupingPolicyScope::Profile,
            operation_id,
            digest(1),
            0,
            AnimeGroupingPolicyChange::Set(AnimeGroupingPreference::KeepMalReleasesSeparate),
        )
        .expect("apply command");
        let applied = node
            .kernel
            .authorize_and_apply_anime_grouping_policy_change(command.clone())
            .expect("apply policy");
        assert_eq!(applied.policy().revision(), 1);
        assert_eq!(applied.affected_records(), 1);
        assert_eq!(
            applied.policy().preference(),
            AnimeGroupingPreference::KeepMalReleasesSeparate
        );
        assert_eq!(
            node.kernel
                .authorize_and_apply_anime_grouping_policy_change(command)
                .expect("replay policy"),
            applied
        );

        let conflict = ApplyAnimeGroupingPolicyChangeCommand::try_new(
            RequestCorrelationId::new_v7(),
            node.access,
            AnimeGroupingPolicyScope::Profile,
            operation_id,
            digest(2),
            0,
            AnimeGroupingPolicyChange::Set(AnimeGroupingPreference::KeepMalReleasesSeparate),
        )
        .expect("conflicting command");
        assert_eq!(
            node.kernel
                .authorize_and_apply_anime_grouping_policy_change(conflict)
                .expect_err("semantic conflict")
                .code(),
            ProblemCode::IdempotencyConflict
        );
    }

    #[test]
    fn client_override_can_return_to_inherited_profile_state() {
        let node = TestNode::new();
        let client_id = node.access.client_id();
        let scope = AnimeGroupingPolicyScope::Client(client_id);
        let first = ApplyAnimeGroupingPolicyChangeCommand::try_new(
            RequestCorrelationId::new_v7(),
            node.access,
            scope,
            OperationId::new_v7(),
            digest(3),
            0,
            AnimeGroupingPolicyChange::Set(AnimeGroupingPreference::KeepKitsuReleasesSeparate),
        )
        .expect("override command");
        let overridden = node
            .kernel
            .authorize_and_apply_anime_grouping_policy_change(first)
            .expect("set override");
        assert_eq!(
            overridden.policy().source(),
            AnimeGroupingPolicySource::ClientOverride
        );

        let inherit = ApplyAnimeGroupingPolicyChangeCommand::try_new(
            RequestCorrelationId::new_v7(),
            node.access,
            scope,
            OperationId::new_v7(),
            digest(4),
            overridden.policy().revision(),
            AnimeGroupingPolicyChange::InheritProfile,
        )
        .expect("inherit command");
        let inherited = node
            .kernel
            .authorize_and_apply_anime_grouping_policy_change(inherit)
            .expect("inherit profile");
        assert_eq!(
            inherited.policy().source(),
            AnimeGroupingPolicySource::ProfileDefault
        );
        assert_eq!(
            inherited.policy().preference(),
            AnimeGroupingPreference::Automatic
        );
    }

    #[test]
    fn missing_scope_and_stale_revision_fail_without_mutation() {
        let node = TestNode::new();
        let other = node.add_profile_with_scopes(&[ScopeKey::ProfileStateWrite]);
        let command = ApplyAnimeGroupingPolicyChangeCommand::try_new(
            RequestCorrelationId::new_v7(),
            other,
            AnimeGroupingPolicyScope::Profile,
            OperationId::new_v7(),
            digest(5),
            7,
            AnimeGroupingPolicyChange::Set(AnimeGroupingPreference::GroupByTvWork),
        )
        .expect("stale command");
        assert_eq!(
            node.kernel
                .authorize_and_apply_anime_grouping_policy_change(command)
                .expect_err("stale revision")
                .code(),
            ProblemCode::IdempotencyConflict
        );
    }

    #[test]
    fn application_client_override_cannot_target_another_client() {
        let node = TestNode::new();
        let query = ReadAnimeGroupingPolicyQuery::new(
            RequestCorrelationId::new_v7(),
            node.access,
            AnimeGroupingPolicyScope::Client(ClientId::new_v7()),
        );
        assert_eq!(
            node.kernel
                .authorize_and_read_anime_grouping_policy(query)
                .expect_err("cross-client read")
                .code(),
            ProblemCode::Forbidden
        );
    }

    #[test]
    fn preview_uses_workspace_keyset_indexes() {
        let node = TestNode::new();
        let connection = node.kernel.inner.connection.lock().expect("connection");
        let plan = |sql: &str| {
            connection
                .prepare(sql)
                .expect("prepare query plan")
                .query_map(
                    params![node.access.workspace_id().to_string(), "", "~", 256],
                    |row| row.get::<_, String>(3),
                )
                .expect("query plan")
                .collect::<Result<Vec<_>, _>>()
                .expect("collect query plan")
                .join("\n")
        };
        let records = plan(
            "EXPLAIN QUERY PLAN SELECT record_id FROM records WHERE workspace_id = ?1 AND status = 'active' AND record_id > ?2 ORDER BY record_id LIMIT ?4",
        );
        assert!(
            records.contains("records_workspace_record_idx"),
            "{records}"
        );
        let identifiers = plan(
            "EXPLAIN QUERY PLAN SELECT identifier.record_id FROM external_identifiers identifier JOIN records record ON record.workspace_id = identifier.workspace_id AND record.record_id = identifier.record_id AND record.status = 'active' WHERE identifier.workspace_id = ?1 AND identifier.record_id >= ?2 AND identifier.record_id <= ?3 ORDER BY identifier.record_id LIMIT ?4",
        );
        assert!(
            identifiers.contains("external_identifiers_record_idx"),
            "{identifiers}"
        );
        let assertions = plan(
            "EXPLAIN QUERY PLAN SELECT assertion.record_id FROM identity_assertions assertion JOIN records record ON record.workspace_id = assertion.workspace_id AND record.record_id = assertion.record_id AND record.status = 'active' WHERE assertion.workspace_id = ?1 AND assertion.record_id >= ?2 AND assertion.record_id <= ?3 ORDER BY assertion.record_id, assertion.assertion_id LIMIT ?4",
        );
        assert!(
            assertions.contains("identity_assertions_record_idx"),
            "{assertions}"
        );
    }

    #[test]
    fn preview_scans_ten_thousand_records_in_bounded_batches() {
        let node = TestNode::new();
        {
            let mut connection = node.kernel.inner.connection.lock().expect("connection");
            let transaction = connection.transaction().expect("transaction");
            let created_at = timestamp(now());
            {
                let mut insert = transaction
                    .prepare(
                        "INSERT INTO records(record_id, workspace_id, grain, status, created_at) VALUES (?1, ?2, 'release', 'active', ?3)",
                    )
                    .expect("prepare record insert");
                for _ in 0..10_000 {
                    insert
                        .execute(params![
                            RecordId::new_v7().to_string(),
                            node.access.workspace_id().to_string(),
                            created_at
                        ])
                        .expect("insert record");
                }
            }
            transaction.commit().expect("commit records");
        }
        let query = PreviewAnimeGroupingPolicyChangeQuery::try_new(
            RequestCorrelationId::new_v7(),
            node.access,
            AnimeGroupingPolicyScope::Profile,
            AnimeGroupingPolicyChange::Set(AnimeGroupingPreference::GroupByTvWork),
            None,
            fasti_application::IdentityImpactPageLimit::try_new(100).expect("page limit"),
        )
        .expect("preview query");
        let impact = node
            .kernel
            .authorize_and_preview_anime_grouping_policy_change(query)
            .expect("preview ten thousand records");
        assert_eq!(impact.total_records(), 10_000);
        assert_eq!(impact.records().len(), 100);
        assert!(impact.next_after_record_id().is_some());
    }
}
