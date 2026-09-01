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
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

const MAX_IDENTITY_ASSERTIONS_PER_RECORD: i64 = 256;
const MAX_IDENTITY_LIFECYCLE_EVENTS_PER_ASSERTION: i64 = 64;
const MAX_IDENTITY_LIFECYCLE_EVENTS_PER_PREVIEW_BATCH: i64 =
    MAX_IDENTITY_ASSERTIONS_PER_RECORD * MAX_IDENTITY_LIFECYCLE_EVENTS_PER_ASSERTION;
const POLICY_PREVIEW_BATCH_SIZE: i64 = 256;

fn preview_lifecycle_split_at(
    record_count: usize,
    lifecycle_event_count: i64,
) -> Result<Option<usize>, ()> {
    if lifecycle_event_count <= MAX_IDENTITY_LIFECYCLE_EVENTS_PER_PREVIEW_BATCH {
        return Ok(None);
    }
    (record_count > 1)
        .then_some(record_count / 2)
        .map(Some)
        .ok_or(())
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PolicyState {
    preference: AnimeGroupingPreference,
    source: AnimeGroupingPolicySource,
    revision: u64,
}

struct StoredPolicyReceipt {
    profile_id: String,
    actor_client_id: String,
    scope_kind: String,
    scope_client_id: Option<String>,
    operation_id: String,
    change_kind: String,
    requested_preference: Option<String>,
    rollback_operation_id: Option<String>,
    previous_preference: String,
    previous_source: String,
    result_preference: String,
    result_source: String,
    result_revision: i64,
    affected_records: i64,
    unresolved_routes: i64,
    possible_season_regroupings: i64,
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

#[allow(clippy::too_many_arguments)]
fn materialize_lifecycle_event(
    assertion_id: IdentityAssertionId,
    sequence: i64,
    previous: String,
    status: String,
    reviewer: String,
    occurred_at: String,
    evidence_digest: Option<String>,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<IdentityAssertionLifecycleEvent> {
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
    .map_err(|_| integrity(capability, correlation_id))
}

#[allow(clippy::too_many_arguments)]
fn load_lifecycle_events_for_record_batch(
    connection: &Connection,
    workspace_id: WorkspaceId,
    first_record_id: RecordId,
    last_record_id: RecordId,
    record_count: usize,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<HashMap<IdentityAssertionId, Vec<IdentityAssertionLifecycleEvent>>> {
    let row_limit = record_count
        .checked_mul(MAX_IDENTITY_ASSERTIONS_PER_RECORD as usize + 1)
        .and_then(|count| {
            count.checked_mul(MAX_IDENTITY_LIFECYCLE_EVENTS_PER_ASSERTION as usize + 1)
        })
        .and_then(|count| count.checked_add(1))
        .and_then(|count| i64::try_from(count).ok())
        .unwrap_or(i64::MAX);
    let mut statement = map_sql(
        connection.prepare(
            r#"
            SELECT event.assertion_id, event.sequence, event.previous_status,
                   event.status, event.reviewer_client_id, event.occurred_at,
                   event.evidence_digest
            FROM identity_assertion_lifecycle_events event
            JOIN identity_assertions assertion
              ON assertion.workspace_id = event.workspace_id
             AND assertion.assertion_id = event.assertion_id
            JOIN records record
              ON record.workspace_id = assertion.workspace_id
             AND record.record_id = assertion.record_id
             AND record.status = 'active'
            WHERE event.workspace_id = ?1
              AND assertion.record_id >= ?2
              AND assertion.record_id <= ?3
            ORDER BY event.assertion_id, event.sequence
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
                first_record_id.to_string(),
                last_record_id.to_string(),
                row_limit
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            },
        ),
        capability,
        correlation_id,
    )?;
    let mut events = HashMap::<IdentityAssertionId, Vec<IdentityAssertionLifecycleEvent>>::new();
    let mut row_count = 0_i64;
    for row in rows {
        let (assertion_id, sequence, previous, status, reviewer, occurred_at, evidence_digest) =
            map_sql(row, capability, correlation_id)?;
        row_count += 1;
        if row_count >= row_limit {
            return Err(Box::new(FastiProblem::capacity_exceeded(
                capability,
                correlation_id,
            )));
        }
        let assertion_id = IdentityAssertionId::from_str(&assertion_id)
            .map_err(|_| integrity(capability, correlation_id))?;
        let lifecycle = events.entry(assertion_id).or_default();
        lifecycle.push(materialize_lifecycle_event(
            assertion_id,
            sequence,
            previous,
            status,
            reviewer,
            occurred_at,
            evidence_digest,
            capability,
            correlation_id,
        )?);
        if lifecycle.len() > MAX_IDENTITY_LIFECYCLE_EVENTS_PER_ASSERTION as usize {
            return Err(Box::new(FastiProblem::capacity_exceeded(
                capability,
                correlation_id,
            )));
        }
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
    let mut lifecycle_events = load_lifecycle_events_for_record_batch(
        connection,
        workspace_id,
        record_id,
        record_id,
        1,
        capability,
        correlation_id,
    )?;

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
        let lifecycle = lifecycle_events
            .remove(&assertion.assertion_id())
            .unwrap_or_default();
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

pub(crate) fn validate_workspace_anime_grouping_policy_receipts(
    connection: &Connection,
    workspace_id: WorkspaceId,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<()> {
    let capability = CapabilityKey::RestoreWorkspace;
    let total_records = parse_u64(
        map_sql(
            connection.query_row(
                "SELECT COUNT(*) FROM records WHERE workspace_id = ?1",
                [workspace_id.to_string()],
                |row| row.get(0),
            ),
            capability,
            correlation_id,
        )?,
        capability,
        correlation_id,
    )?;
    let mut statement = map_sql(
        connection.prepare(
            r#"
            SELECT profile_id, actor_client_id, scope_kind, scope_client_id,
                   operation_id, change_kind,
                   requested_preference, rollback_operation_id,
                   previous_preference, previous_source, result_preference,
                   result_source, result_revision, affected_records,
                   unresolved_routes, possible_season_regroupings
            FROM anime_grouping_policy_receipts
            WHERE workspace_id = ?1
            ORDER BY CASE scope_kind WHEN 'profile' THEN 0 ELSE 1 END,
                     profile_id, scope_client_id, result_revision, operation_id
            "#,
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map([workspace_id.to_string()], |row| {
            Ok(StoredPolicyReceipt {
                profile_id: row.get(0)?,
                actor_client_id: row.get(1)?,
                scope_kind: row.get(2)?,
                scope_client_id: row.get(3)?,
                operation_id: row.get(4)?,
                change_kind: row.get(5)?,
                requested_preference: row.get(6)?,
                rollback_operation_id: row.get(7)?,
                previous_preference: row.get(8)?,
                previous_source: row.get(9)?,
                result_preference: row.get(10)?,
                result_source: row.get(11)?,
                result_revision: row.get(12)?,
                affected_records: row.get(13)?,
                unresolved_routes: row.get(14)?,
                possible_season_regroupings: row.get(15)?,
            })
        }),
        capability,
        correlation_id,
    )?;
    let receipts = rows
        .map(|row| map_sql(row, capability, correlation_id))
        .collect::<ApplicationResult<Vec<_>>>()?;
    drop(statement);
    let mut receipt_counts = HashMap::<String, u64>::new();
    let mut inherited_client_revisions = HashMap::<String, HashSet<u64>>::new();
    for receipt in &receipts {
        let count = receipt_counts
            .entry(receipt.profile_id.clone())
            .or_default();
        *count = count
            .checked_add(1)
            .ok_or_else(|| integrity(capability, correlation_id))?;
        if receipt.scope_kind == "client" && receipt.result_source == "profile_default" {
            inherited_client_revisions
                .entry(receipt.profile_id.clone())
                .or_default()
                .insert(parse_u64(
                    receipt.result_revision,
                    capability,
                    correlation_id,
                )?);
        }
    }

    let default_state = PolicyState {
        preference: AnimeGroupingPreference::Automatic,
        source: AnimeGroupingPolicySource::ProfileDefault,
        revision: 0,
    };
    let mut profiles = HashMap::<ProfileId, PolicyState>::new();
    let mut clients = HashMap::<(ProfileId, ClientId), PolicyState>::new();
    let mut rollback_states = HashMap::<
        (ProfileId, Option<ClientId>, OperationId),
        (AnimeGroupingPreference, AnimeGroupingPolicySource),
    >::new();
    let mut profile_history = HashMap::<ProfileId, Vec<PolicyState>>::new();

    for receipt in receipts {
        let profile_id = ProfileId::from_str(&receipt.profile_id)
            .map_err(|_| integrity(capability, correlation_id))?;
        let actor_client_id = ClientId::from_str(&receipt.actor_client_id)
            .map_err(|_| integrity(capability, correlation_id))?;
        let operation_id = OperationId::from_str(&receipt.operation_id)
            .map_err(|_| integrity(capability, correlation_id))?;
        let scope_client_id = receipt
            .scope_client_id
            .as_deref()
            .map(ClientId::from_str)
            .transpose()
            .map_err(|_| integrity(capability, correlation_id))?;
        let scope = match (receipt.scope_kind.as_str(), scope_client_id) {
            ("profile", None) => AnimeGroupingPolicyScope::Profile,
            ("client", Some(client_id)) if client_id == actor_client_id => {
                AnimeGroupingPolicyScope::Client(client_id)
            }
            _ => return Err(integrity(capability, correlation_id)),
        };
        let profile_state = *profiles.get(&profile_id).unwrap_or(&default_state);
        let history = profile_history
            .entry(profile_id)
            .or_insert_with(|| vec![default_state]);
        let client_state = scope
            .client_id()
            .and_then(|client_id| clients.get(&(profile_id, client_id)).copied());
        let prior_state = match scope {
            AnimeGroupingPolicyScope::Profile => Some(profile_state),
            AnimeGroupingPolicyScope::Client(_) => client_state,
        };
        let previous_preference = preference(&receipt.previous_preference)
            .ok_or_else(|| integrity(capability, correlation_id))?;
        let previous_source = policy_source(&receipt.previous_source)
            .ok_or_else(|| integrity(capability, correlation_id))?;
        let previous_is_valid = match (scope, prior_state) {
            (AnimeGroupingPolicyScope::Profile, Some(previous)) => {
                previous.preference == previous_preference && previous.source == previous_source
            }
            (AnimeGroupingPolicyScope::Client(_), None)
            | (
                AnimeGroupingPolicyScope::Client(_),
                Some(PolicyState {
                    source: AnimeGroupingPolicySource::ProfileDefault,
                    ..
                }),
            ) => {
                previous_source == AnimeGroupingPolicySource::ProfileDefault
                    && history
                        .iter()
                        .any(|state| state.preference == previous_preference)
            }
            (AnimeGroupingPolicyScope::Client(_), Some(previous)) => {
                previous.preference == previous_preference && previous.source == previous_source
            }
            _ => false,
        };
        if !previous_is_valid {
            return Err(integrity(capability, correlation_id));
        }

        let change = match (
            receipt.change_kind.as_str(),
            receipt.requested_preference.as_deref(),
            receipt.rollback_operation_id.as_deref(),
        ) {
            ("set", Some(value), None) => AnimeGroupingPolicyChange::Set(
                preference(value).ok_or_else(|| integrity(capability, correlation_id))?,
            ),
            ("inherit_profile", None, None)
                if matches!(scope, AnimeGroupingPolicyScope::Client(_)) =>
            {
                AnimeGroupingPolicyChange::InheritProfile
            }
            ("rollback", None, Some(value)) => AnimeGroupingPolicyChange::Rollback {
                applied_operation_id: OperationId::from_str(value)
                    .map_err(|_| integrity(capability, correlation_id))?,
            },
            _ => return Err(integrity(capability, correlation_id)),
        };
        let result_preference = preference(&receipt.result_preference)
            .ok_or_else(|| integrity(capability, correlation_id))?;
        let result_source = policy_source(&receipt.result_source)
            .ok_or_else(|| integrity(capability, correlation_id))?;
        let expected_result = match change {
            AnimeGroupingPolicyChange::Set(value) => Some((
                value,
                match scope {
                    AnimeGroupingPolicyScope::Profile => AnimeGroupingPolicySource::ProfileDefault,
                    AnimeGroupingPolicyScope::Client(_) => {
                        AnimeGroupingPolicySource::ClientOverride
                    }
                },
            )),
            AnimeGroupingPolicyChange::InheritProfile
                if result_source == AnimeGroupingPolicySource::ProfileDefault
                    && history
                        .iter()
                        .any(|state| state.preference == result_preference) =>
            {
                match prior_state {
                    Some(previous)
                        if previous.source == AnimeGroupingPolicySource::ProfileDefault
                            && previous.preference != result_preference =>
                    {
                        None
                    }
                    _ => Some((result_preference, result_source)),
                }
            }
            AnimeGroupingPolicyChange::Rollback {
                applied_operation_id,
            } => rollback_states
                .get(&(profile_id, scope.client_id(), applied_operation_id))
                .copied(),
            AnimeGroupingPolicyChange::InheritProfile => None,
        };
        let result_revision = parse_u64(receipt.result_revision, capability, correlation_id)?;
        let predecessor_revision = result_revision.checked_sub(1);
        let exact_profile_predecessor = predecessor_revision.is_some_and(|predecessor| {
            history.iter().any(|state| {
                state.preference == previous_preference && state.revision == predecessor
            })
        });
        let revision_is_valid = match (scope, prior_state) {
            (AnimeGroupingPolicyScope::Profile, Some(previous)) => predecessor_revision
                .is_some_and(|predecessor| {
                    predecessor == previous.revision
                        || (predecessor > previous.revision
                            && inherited_client_revisions
                                .get(&receipt.profile_id)
                                .is_some_and(|revisions| revisions.contains(&predecessor)))
                }),
            (AnimeGroupingPolicyScope::Client(_), None) => exact_profile_predecessor,
            (AnimeGroupingPolicyScope::Client(_), Some(previous))
                if previous.source == AnimeGroupingPolicySource::ClientOverride =>
            {
                previous
                    .revision
                    .checked_add(1)
                    .is_some_and(|next| result_revision == next)
            }
            (AnimeGroupingPolicyScope::Client(_), Some(previous)) => predecessor_revision
                .is_some_and(|predecessor| {
                    predecessor >= previous.revision
                        && if predecessor == previous.revision {
                            history.iter().any(|state| {
                                state.preference == previous_preference
                                    && state.revision <= predecessor
                            })
                        } else {
                            exact_profile_predecessor
                        }
                }),
            _ => false,
        };
        if result_revision
            > receipt_counts
                .get(&receipt.profile_id)
                .copied()
                .unwrap_or_default()
            || !revision_is_valid
            || expected_result != Some((result_preference, result_source))
        {
            return Err(integrity(capability, correlation_id));
        }
        let affected_records = parse_u64(receipt.affected_records, capability, correlation_id)?;
        let unresolved_routes = parse_u64(receipt.unresolved_routes, capability, correlation_id)?;
        let possible_season_regroupings = parse_u64(
            receipt.possible_season_regroupings,
            capability,
            correlation_id,
        )?;
        if affected_records > total_records
            || unresolved_routes > total_records
            || possible_season_regroupings > affected_records
        {
            return Err(integrity(capability, correlation_id));
        }

        rollback_states.insert(
            (profile_id, scope.client_id(), operation_id),
            (previous_preference, previous_source),
        );
        let result = PolicyState {
            preference: result_preference,
            source: result_source,
            revision: result_revision,
        };
        match scope {
            AnimeGroupingPolicyScope::Profile => {
                profiles.insert(profile_id, result);
                history.push(result);
            }
            AnimeGroupingPolicyScope::Client(client_id) => {
                clients.insert((profile_id, client_id), result);
            }
        }
    }

    let mut stored_profiles = HashMap::new();
    let mut statement = map_sql(
        connection.prepare(
            "SELECT profile_id, preference, revision FROM profile_anime_grouping_policies WHERE workspace_id = ?1",
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map([workspace_id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        }),
        capability,
        correlation_id,
    )?;
    for row in rows {
        let (profile_id, value, revision) = map_sql(row, capability, correlation_id)?;
        stored_profiles.insert(
            ProfileId::from_str(&profile_id).map_err(|_| integrity(capability, correlation_id))?,
            PolicyState {
                preference: preference(&value)
                    .ok_or_else(|| integrity(capability, correlation_id))?,
                source: AnimeGroupingPolicySource::ProfileDefault,
                revision: parse_u64(revision, capability, correlation_id)?,
            },
        );
    }
    drop(statement);
    if stored_profiles != profiles {
        return Err(integrity(capability, correlation_id));
    }

    let mut stored_clients = HashMap::new();
    let mut statement = map_sql(
        connection.prepare(
            "SELECT profile_id, client_id, preference, revision FROM client_anime_grouping_policies WHERE workspace_id = ?1",
        ),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map([workspace_id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)?,
            ))
        }),
        capability,
        correlation_id,
    )?;
    for row in rows {
        let (profile_id, client_id, value, revision) = map_sql(row, capability, correlation_id)?;
        let profile_id =
            ProfileId::from_str(&profile_id).map_err(|_| integrity(capability, correlation_id))?;
        let client_id =
            ClientId::from_str(&client_id).map_err(|_| integrity(capability, correlation_id))?;
        let profile = *profiles.get(&profile_id).unwrap_or(&default_state);
        let state = match value {
            Some(value) => PolicyState {
                preference: preference(&value)
                    .ok_or_else(|| integrity(capability, correlation_id))?,
                source: AnimeGroupingPolicySource::ClientOverride,
                revision: parse_u64(revision, capability, correlation_id)?,
            },
            None => PolicyState {
                preference: profile.preference,
                source: AnimeGroupingPolicySource::ProfileDefault,
                revision: parse_u64(revision, capability, correlation_id)?,
            },
        };
        stored_clients.insert((profile_id, client_id), state);
    }
    drop(statement);
    for ((profile_id, _), state) in &mut clients {
        if state.source == AnimeGroupingPolicySource::ProfileDefault {
            state.preference = profiles
                .get(profile_id)
                .unwrap_or(&default_state)
                .preference;
        }
    }
    if stored_clients != clients {
        return Err(integrity(capability, correlation_id));
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
    let lifecycle_event_count = map_sql(
        connection.query_row(
            r#"
            SELECT COUNT(*) FROM (
                SELECT 1
                FROM identity_assertion_lifecycle_events event
                JOIN identity_assertions assertion
                  ON assertion.workspace_id = event.workspace_id
                 AND assertion.assertion_id = event.assertion_id
                JOIN records record
                  ON record.workspace_id = assertion.workspace_id
                 AND record.record_id = assertion.record_id
                 AND record.status = 'active'
                WHERE event.workspace_id = ?1
                  AND assertion.record_id >= ?2
                  AND assertion.record_id <= ?3
                LIMIT ?4
            )
            "#,
            params![
                workspace_id.to_string(),
                first.to_string(),
                last.to_string(),
                MAX_IDENTITY_LIFECYCLE_EVENTS_PER_PREVIEW_BATCH + 1
            ],
            |row| row.get::<_, i64>(0),
        ),
        capability,
        correlation_id,
    )?;
    let split_at = match preview_lifecycle_split_at(record_ids.len(), lifecycle_event_count) {
        Ok(split_at) => split_at,
        Err(()) => {
            return Err(Box::new(FastiProblem::capacity_exceeded(
                capability,
                correlation_id,
            )));
        }
    };
    if let Some(split_at) = split_at {
        let (left, right) = record_ids.split_at(split_at);
        let mut evidence = load_preview_route_evidence(
            connection,
            workspace_id,
            left,
            capability,
            correlation_id,
        )?;
        evidence.extend(load_preview_route_evidence(
            connection,
            workspace_id,
            right,
            capability,
            correlation_id,
        )?);
        return Ok(evidence);
    }
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
        record_evidence.push(IdentityRouteEvidence::direct(
            ExternalIdentifierClaim::try_new(
                namespace,
                Grain::from_str(&grain).map_err(|_| integrity(capability, correlation_id))?,
                value,
            )
            .map_err(|_| integrity(capability, correlation_id))?,
        ));
        if record_evidence.len() > MAX_IDENTITY_CLAIMS {
            return Err(Box::new(FastiProblem::capacity_exceeded(
                capability,
                correlation_id,
            )));
        }
    }
    drop(identifiers);
    let mut lifecycle_events = load_lifecycle_events_for_record_batch(
        connection,
        workspace_id,
        *first,
        *last,
        record_ids.len(),
        capability,
        correlation_id,
    )?;

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
        let lifecycle = lifecycle_events
            .remove(&assertion.assertion_id())
            .unwrap_or_default();
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
    profile_id: ProfileId,
    actor_client_id: ClientId,
    command: &ApplyAnimeGroupingPolicyChangeCommand,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
) -> ApplicationResult<Option<ReceiptState>> {
    let row = map_sql(
        connection
            .query_row(
                r#"
                SELECT profile_id, scope_kind, scope_client_id, semantic_digest,
                       previous_preference, previous_source,
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
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, i64>(9)?,
                        row.get::<_, i64>(10)?,
                        row.get::<_, i64>(11)?,
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
    let (scope_kind, scope_client_id) = receipt_scope(command.scope());
    if row.0 != profile_id.to_string()
        || row.1 != scope_kind
        || row.2 != scope_client_id
        || row.3 != command.semantic_digest().as_str()
    {
        return Err(idempotency_conflict(capability, correlation_id));
    }
    Ok(Some(ReceiptState {
        previous_preference: preference(&row.4)
            .ok_or_else(|| integrity(capability, correlation_id))?,
        previous_source: policy_source(&row.5)
            .ok_or_else(|| integrity(capability, correlation_id))?,
        result_preference: preference(&row.6)
            .ok_or_else(|| integrity(capability, correlation_id))?,
        result_source: policy_source(&row.7)
            .ok_or_else(|| integrity(capability, correlation_id))?,
        result_revision: parse_u64(row.8, capability, correlation_id)?,
        affected_records: parse_u64(row.9, capability, correlation_id)?,
        unresolved_routes: parse_u64(row.10, capability, correlation_id)?,
        possible_season_regroupings: parse_u64(row.11, capability, correlation_id)?,
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
            authorized.profile_id(),
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
        let stored_client_revision = match command.scope() {
            AnimeGroupingPolicyScope::Profile => None,
            AnimeGroupingPolicyScope::Client(client_id) => map_sql(
                transaction
                    .query_row(
                        r#"
                        SELECT revision
                        FROM client_anime_grouping_policies
                        WHERE workspace_id = ?1 AND profile_id = ?2 AND client_id = ?3
                        "#,
                        params![
                            authorized.workspace_id().to_string(),
                            authorized.profile_id().to_string(),
                            client_id.to_string()
                        ],
                        |row| row.get::<_, i64>(0),
                    )
                    .optional(),
                capability,
                correlation_id,
            )?
            .map(|revision| parse_u64(revision, capability, correlation_id))
            .transpose()?,
        };
        let (result_preference, result_source) = proposed_state(
            &transaction,
            authorized.workspace_id(),
            authorized.profile_id(),
            command.scope(),
            command.change(),
            capability,
            correlation_id,
        )?;
        let inherited_client_revision = match command.scope() {
            AnimeGroupingPolicyScope::Profile => parse_u64(
                map_sql(
                    transaction.query_row(
                        r#"
                        SELECT COALESCE(MAX(revision), 0)
                        FROM client_anime_grouping_policies
                        WHERE workspace_id = ?1 AND profile_id = ?2
                          AND preference IS NULL
                        "#,
                        params![
                            authorized.workspace_id().to_string(),
                            authorized.profile_id().to_string()
                        ],
                        |row| row.get(0),
                    ),
                    capability,
                    correlation_id,
                )?,
                capability,
                correlation_id,
            )?,
            AnimeGroupingPolicyScope::Client(_) => 0,
        };
        let next_revision = current
            .revision
            .max(inherited_client_revision)
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
                let changed = map_sql(
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
                if changed != 1 {
                    return Err(idempotency_conflict(capability, correlation_id));
                }
            }
            AnimeGroupingPolicyScope::Client(client_id) => {
                let stored_preference = (result_source
                    == AnimeGroupingPolicySource::ClientOverride)
                    .then_some(result_preference.as_str());
                let changed = map_sql(
                    transaction.execute(
                        r#"
                        INSERT INTO client_anime_grouping_policies(
                            workspace_id, profile_id, client_id, preference, revision, updated_at
                        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                        ON CONFLICT(workspace_id, profile_id, client_id) DO UPDATE SET
                            preference = excluded.preference,
                            revision = excluded.revision,
                            updated_at = excluded.updated_at
                        WHERE client_anime_grouping_policies.revision = ?7
                        "#,
                        params![
                            authorized.workspace_id().to_string(),
                            authorized.profile_id().to_string(),
                            client_id.to_string(),
                            stored_preference,
                            i64::try_from(next_revision).unwrap_or(i64::MAX),
                            timestamp(now()),
                            i64::try_from(stored_client_revision.unwrap_or(current.revision))
                                .unwrap_or(i64::MAX)
                        ],
                    ),
                    capability,
                    correlation_id,
                )?;
                if changed != 1 {
                    return Err(idempotency_conflict(capability, correlation_id));
                }
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
        AcceptObservationCommand, AttachIdentifierCommand, CreateRecordCommand, IdentityPort,
        ListRecordsQuery, ObservationAcceptancePort, RegisterNamespaceDefinitionCommand, ScopeKey,
    };
    use fasti_domain::{ClaimedTrust, NamespaceDefinition, NamespaceLicencePosture, ObservedAt};

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
        let mal_claim = ExternalIdentifierClaim::try_new("mal.anime", Grain::Release, "49894")
            .expect("MAL identifier");
        node.kernel
            .authorize_and_accept(
                AcceptObservationCommand::new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    OperationId::new_v7(),
                    None,
                    ObservedAt::parse("2026-08-29T10:30:00Z", ClaimedTrust::DeviceObserved)
                        .expect("observed_at"),
                    node.upload(b"anime grouping policy Chronicle invariant"),
                )
                .with_identity_clues(vec![mal_claim], Some(Grain::Release)),
            )
            .expect("accept observation resolving to the anime record");
        let records_before = node
            .kernel
            .list_records(ListRecordsQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
            ))
            .expect("list records before policy change")
            .into_records();
        assert_eq!(records_before.len(), 1);
        assert_eq!(records_before[0].record_id(), record_id);
        assert!(records_before[0].latest_activity().is_some());
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
        let records_after = node
            .kernel
            .list_records(ListRecordsQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
            ))
            .expect("list records after policy change")
            .into_records();
        assert_eq!(records_after, records_before);

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
    fn policy_replay_is_bound_to_the_original_profile_and_scope() {
        let node = TestNode::new();
        let operation_id = OperationId::new_v7();
        let semantic_digest = digest(42);
        node.kernel
            .authorize_and_apply_anime_grouping_policy_change(
                ApplyAnimeGroupingPolicyChangeCommand::try_new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    AnimeGroupingPolicyScope::Profile,
                    operation_id,
                    semantic_digest.clone(),
                    0,
                    AnimeGroupingPolicyChange::Set(
                        AnimeGroupingPreference::KeepMalReleasesSeparate,
                    ),
                )
                .expect("original policy command"),
            )
            .expect("apply original profile policy");

        let other_profile = node
            .add_profile_with_scopes(&[ScopeKey::ProfileStateRead, ScopeKey::ProfileStateWrite]);
        let cross_profile = ApplyAnimeGroupingPolicyChangeCommand::try_new(
            RequestCorrelationId::new_v7(),
            other_profile,
            AnimeGroupingPolicyScope::Profile,
            operation_id,
            semantic_digest.clone(),
            0,
            AnimeGroupingPolicyChange::Set(AnimeGroupingPreference::KeepMalReleasesSeparate),
        )
        .expect("cross-profile replay");
        assert_eq!(
            node.kernel
                .authorize_and_apply_anime_grouping_policy_change(cross_profile)
                .expect_err("receipt cannot cross profiles")
                .code(),
            ProblemCode::IdempotencyConflict
        );

        let cross_scope = ApplyAnimeGroupingPolicyChangeCommand::try_new(
            RequestCorrelationId::new_v7(),
            node.access,
            AnimeGroupingPolicyScope::Client(node.access.client_id()),
            operation_id,
            semantic_digest,
            0,
            AnimeGroupingPolicyChange::Set(AnimeGroupingPreference::KeepMalReleasesSeparate),
        )
        .expect("cross-scope replay");
        assert_eq!(
            node.kernel
                .authorize_and_apply_anime_grouping_policy_change(cross_scope)
                .expect_err("receipt cannot cross scopes")
                .code(),
            ProblemCode::IdempotencyConflict
        );

        let untouched = node
            .kernel
            .authorize_and_read_anime_grouping_policy(ReadAnimeGroupingPolicyQuery::new(
                RequestCorrelationId::new_v7(),
                other_profile,
                AnimeGroupingPolicyScope::Profile,
            ))
            .expect("read untouched profile");
        assert_eq!(untouched.policy().revision(), 0);
        assert_eq!(
            untouched.policy().preference(),
            AnimeGroupingPreference::Automatic
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

        node.kernel
            .authorize_and_apply_anime_grouping_policy_change(
                ApplyAnimeGroupingPolicyChangeCommand::try_new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    AnimeGroupingPolicyScope::Profile,
                    OperationId::new_v7(),
                    digest(5),
                    0,
                    AnimeGroupingPolicyChange::Set(
                        AnimeGroupingPreference::KeepMalReleasesSeparate,
                    ),
                )
                .expect("profile change"),
            )
            .expect("change inherited profile policy");
        let stale = ApplyAnimeGroupingPolicyChangeCommand::try_new(
            RequestCorrelationId::new_v7(),
            node.access,
            scope,
            OperationId::new_v7(),
            digest(6),
            inherited.policy().revision(),
            AnimeGroupingPolicyChange::Set(AnimeGroupingPreference::GroupByTvWork),
        )
        .expect("stale inherited command");
        assert_eq!(
            node.kernel
                .authorize_and_apply_anime_grouping_policy_change(stale)
                .expect_err("profile change invalidates inherited client revision")
                .code(),
            ProblemCode::IdempotencyConflict
        );
        let refreshed = node
            .kernel
            .authorize_and_read_anime_grouping_policy(ReadAnimeGroupingPolicyQuery::new(
                RequestCorrelationId::new_v7(),
                node.access,
                scope,
            ))
            .expect("read inherited client after profile change");
        assert_eq!(
            refreshed.policy().revision(),
            inherited.policy().revision() + 1
        );
        assert_eq!(
            refreshed.policy().preference(),
            AnimeGroupingPreference::KeepMalReleasesSeparate
        );
        let refreshed_override = node
            .kernel
            .authorize_and_apply_anime_grouping_policy_change(
                ApplyAnimeGroupingPolicyChangeCommand::try_new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    scope,
                    OperationId::new_v7(),
                    digest(7),
                    refreshed.policy().revision(),
                    AnimeGroupingPolicyChange::Set(AnimeGroupingPreference::GroupByTvWork),
                )
                .expect("refreshed override command"),
            )
            .expect("replace inherited state after profile advance");
        assert_eq!(
            refreshed_override.policy().source(),
            AnimeGroupingPolicySource::ClientOverride
        );
        assert_eq!(
            refreshed_override.policy().revision(),
            refreshed.policy().revision() + 1
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

    #[test]
    fn preview_accepts_the_exact_identifier_limit() {
        let node = TestNode::new();
        node.kernel
            .register_namespace_definition(RegisterNamespaceDefinitionCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                NamespaceDefinition::try_new(
                    "limit.ids",
                    "Limit IDs",
                    [Grain::Release],
                    ".+",
                    "identity",
                    NamespaceLicencePosture::IdentifiersOnly,
                )
                .expect("namespace definition"),
            ))
            .expect("register namespace");
        let record_id = node
            .kernel
            .create_record(CreateRecordCommand::new(
                RequestCorrelationId::new_v7(),
                node.access,
                Grain::Release,
            ))
            .expect("create record")
            .record_id();
        for index in 0..MAX_IDENTITY_CLAIMS {
            node.kernel
                .attach_identifier(AttachIdentifierCommand::new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    record_id,
                    ExternalIdentifierClaim::try_new(
                        "limit.ids",
                        Grain::Release,
                        format!("id-{index}"),
                    )
                    .expect("identifier"),
                ))
                .expect("attach identifier");
        }

        let impact = node
            .kernel
            .authorize_and_preview_anime_grouping_policy_change(
                PreviewAnimeGroupingPolicyChangeQuery::try_new(
                    RequestCorrelationId::new_v7(),
                    node.access,
                    AnimeGroupingPolicyScope::Profile,
                    AnimeGroupingPolicyChange::Set(AnimeGroupingPreference::GroupByTvWork),
                    None,
                    fasti_application::IdentityImpactPageLimit::try_new(1).expect("page limit"),
                )
                .expect("preview query"),
            )
            .expect("preview at identifier limit");
        assert_eq!(impact.total_records(), 1);
    }

    #[test]
    fn lifecycle_batch_loader_rejects_its_truncation_sentinel() {
        let node = TestNode::new();
        let record_id = create_anime_record(&node);
        let assertion_id = IdentityAssertionId::new_v7();
        let connection = node.kernel.inner.connection.lock().expect("connection");
        let source_identifier_id: String = connection
            .query_row(
                "SELECT external_identifier_id FROM external_identifiers WHERE workspace_id = ?1 AND record_id = ?2 ORDER BY external_identifier_id LIMIT 1",
                params![
                    node.access.workspace_id().to_string(),
                    record_id.to_string()
                ],
                |row| row.get(0),
            )
            .expect("source identifier");
        connection
            .execute(
                r#"
                INSERT INTO identity_assertions(
                    assertion_id, workspace_id, record_id, source_external_identifier_id,
                    target_namespace, target_grain, target_value, relation, coverage_json,
                    episode_links_json, evidence_class, evidence_json, id_source,
                    source_version, authority, reasoning, initial_status, created_at
                ) VALUES (
                    ?1, ?2, ?3, ?4, 'imdb.title', 'release', 'tt-sentinel', 'exact',
                    '[]', '[]', 'verified',
                    '[{"method":"human_verified","observed_source":"test","derivation_root":null,"reviewer":null,"observed_at":"2026-08-30","evidence_id":null}]',
                    'test-fixture', NULL, NULL, NULL, 'candidate', ?5
                )
                "#,
                params![
                    assertion_id.to_string(),
                    node.access.workspace_id().to_string(),
                    record_id.to_string(),
                    source_identifier_id,
                    "2026-08-31T12:00:00.000000Z",
                ],
            )
            .expect("identity assertion");
        connection
            .execute(
                "INSERT INTO identity_assertion_lifecycle_events(workspace_id, assertion_id, sequence, previous_status, status, reviewer_client_id, occurred_at, evidence_digest) VALUES (?1, ?2, 1, 'candidate', 'accepted', ?3, ?4, NULL)",
                params![
                    node.access.workspace_id().to_string(),
                    assertion_id.to_string(),
                    node.access.client_id().to_string(),
                    "2026-08-31T12:00:01.000000Z",
                ],
            )
            .expect("identity lifecycle event");

        let problem = load_lifecycle_events_for_record_batch(
            &connection,
            node.access.workspace_id(),
            record_id,
            record_id,
            0,
            CapabilityKey::PreviewAnimeGroupingPolicyChange,
            RequestCorrelationId::new_v7(),
        )
        .expect_err("sentinel row must fail closed");
        assert_eq!(problem.code(), ProblemCode::CapacityExceeded);
    }

    #[test]
    fn preview_lifecycle_batches_split_before_dense_event_materialization() {
        assert_eq!(
            preview_lifecycle_split_at(
                POLICY_PREVIEW_BATCH_SIZE as usize,
                MAX_IDENTITY_LIFECYCLE_EVENTS_PER_PREVIEW_BATCH,
            ),
            Ok(None)
        );
        assert_eq!(
            preview_lifecycle_split_at(
                POLICY_PREVIEW_BATCH_SIZE as usize,
                MAX_IDENTITY_LIFECYCLE_EVENTS_PER_PREVIEW_BATCH + 1,
            ),
            Ok(Some(POLICY_PREVIEW_BATCH_SIZE as usize / 2))
        );
        assert_eq!(
            preview_lifecycle_split_at(1, MAX_IDENTITY_LIFECYCLE_EVENTS_PER_PREVIEW_BATCH + 1,),
            Err(())
        );
    }
}
