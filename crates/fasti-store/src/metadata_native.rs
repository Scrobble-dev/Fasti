//! Intrinsic publication facts select observations before interpreting values.

use super::*;

/// Reuse permission and lifecycle cannot expose an older positive observation.
/// Locale preference may choose provenance, but cannot hide conflicting facts
/// about the same intrinsic publication property.
pub(super) fn restrict_to_latest_observations(
    claims: &[PolicyBoundFieldClaim],
    allowed: &mut [bool],
    read_at: chrono::DateTime<chrono::Utc>,
) {
    let mut latest = BTreeMap::new();
    for (claim, _) in claims {
        if let Some(variant) = metadata_variant(claim.provenance()) {
            latest
                .entry(variant)
                .and_modify(|at: &mut chrono::DateTime<chrono::Utc>| {
                    *at = (*at).max(claim.fetched_at());
                })
                .or_insert(claim.fetched_at());
        }
    }
    let mut value = None;
    let mut blocked = false;
    for ((claim, _), permitted) in claims.iter().zip(allowed.iter_mut()) {
        let selected = metadata_variant(claim.provenance())
            .and_then(|variant| latest.get(&variant))
            == Some(&claim.fetched_at());
        if !selected {
            *permitted = false;
            continue;
        }
        let status = claim.status_at(read_at);
        blocked |= !*permitted
            || claim.fetched_at() > read_at
            || !(status.can_project_fresh() || status.can_project_last_known_good());
        if let Some(previous) = value {
            blocked |= previous != claim.value();
        } else {
            value = Some(claim.value());
        }
    }
    if blocked {
        allowed.fill(false);
    }
}

// Rank only narrow observation keys across complete selected-Record history.
// Do not filter by value, policy or lifecycle before choosing the newest keys.
// The ordered coroutine keeps payloads out of the ranking/sorting work.
pub(super) const SELECT_PUBLICATION_OBSERVATIONS: &str = r#"
    WITH page_records AS (
        SELECT record_id FROM records
        WHERE workspace_id = ?1 AND record_id IN (SELECT value FROM json_each(?2))
    ), ranked_keys AS (
        SELECT p.record_id, p.field_key, p.claim_id, p.source, p.fetched_at,
               DENSE_RANK() OVER (
                   PARTITION BY p.record_id, p.provider_id, p.source,
                                p.source_record_id, lower(c.locale), upper(p.region)
                   ORDER BY p.fetched_at DESC
               ) AS observation_rank
        FROM page_records page
        CROSS JOIN metadata_claim_provenance p
            INDEXED BY metadata_claim_provenance_recent_idx
          ON p.workspace_id = ?1 AND p.record_id = page.record_id AND p.field_key = ?3
        CROSS JOIN metadata_field_claims c
            INDEXED BY metadata_field_claims_record_field_idx
          ON c.workspace_id = ?1 AND c.record_id = p.record_id
         AND c.field_key = p.field_key AND c.source = p.source AND c.fetched_at = p.fetched_at
    )
    SELECT selected.record_id, selected.field_key, p.claim_id,
           selected.source, c.value, c.locale, p.provider_id, p.source_record_id,
           p.region, p.source_version, p.evidence_digest, p.provenance_state,
           selected.fetched_at, c.expires_at,
           COALESCE((SELECT lifecycle.status FROM metadata_claim_lifecycle_events lifecycle
                     WHERE lifecycle.claim_id = p.claim_id
                     ORDER BY lifecycle.sequence DESC LIMIT 1), p.initial_status),
           registered.claim_id, registered.response_policy_json
    FROM (
        SELECT * FROM ranked_keys WHERE observation_rank = 1
        ORDER BY record_id, fetched_at DESC, source DESC LIMIT -1
    ) selected
    CROSS JOIN metadata_field_claims c
      ON c.workspace_id = ?1 AND c.record_id = selected.record_id
     AND c.field_key = selected.field_key AND c.source = selected.source
     AND c.fetched_at = selected.fetched_at
    CROSS JOIN metadata_claim_provenance p
      ON p.workspace_id = ?1 AND p.claim_id = selected.claim_id
    LEFT JOIN metadata_claims registered
      ON registered.workspace_id = ?1 AND registered.claim_id = selected.claim_id
     AND registered.record_id = selected.record_id AND registered.claim_kind = 'field'
    ORDER BY selected.record_id, selected.fetched_at DESC, selected.source DESC
"#;

/// Existing authorized callers supply exact selected IDs. Keep one bounded
/// latest-variant group in memory; old history does not consume its payload cap.
#[allow(clippy::too_many_arguments)]
pub(super) fn load_publication_observations(
    connection: &Connection,
    workspace_id: WorkspaceId,
    record_ids: &[RecordId],
    field: &str,
    capability: CapabilityKey,
    correlation_id: RequestCorrelationId,
    read_at: chrono::DateTime<chrono::Utc>,
    live_claim: Option<MetadataClaimId>,
) -> ApplicationResult<HashMap<RecordId, Vec<FieldClaim>>> {
    let ids = selected_record_ids_json(record_ids, capability, correlation_id)?;
    if !fasti_application::is_native_publication_field(field) {
        return Err(receipt_integrity(capability, correlation_id));
    }
    let fields = serde_json::to_string(&[field])
        .map_err(|_| receipt_integrity(capability, correlation_id))?;
    let mut policies = map_sql(
        connection.prepare(SELECT_KNOWN_FIELD_POLICIES),
        capability,
        correlation_id,
    )?;
    // Native selection has already omitted old observations, so the latest
    // known reuse restriction is needed even for histories below the old cap.
    let mut known = known_metadata_policies(
        &mut policies,
        params![workspace_id.to_string(), ids, fields, 0],
        capability,
        correlation_id,
    )?
    .peekable();
    let mut statement = map_sql(
        connection.prepare(SELECT_PUBLICATION_OBSERVATIONS),
        capability,
        correlation_id,
    )?;
    let rows = map_sql(
        statement.query_map(
            params![workspace_id.to_string(), ids, field],
            PersistedFieldClaimRow::read,
        ),
        capability,
        correlation_id,
    )?;
    let mut output = HashMap::new();
    let mut group = None;
    let mut claims = Vec::new();
    for row in rows {
        let ((record, key), claim) =
            map_sql(row, capability, correlation_id)?.decode(capability, correlation_id)?;
        if key.as_str() != field || !record_ids.contains(&record) {
            return Err(receipt_integrity(capability, correlation_id));
        }
        if group != Some(record) {
            if let Some(previous) = group.replace(record) {
                output.insert(
                    previous,
                    reusable_field_claims(
                        &mut claims,
                        read_at,
                        (previous, field),
                        &mut known,
                        live_claim,
                    )?,
                );
            }
        }
        if claims.len() >= MAX_EFFECTIVE_FIELD_CLAIMS as usize {
            return Err(receipt_integrity(capability, correlation_id));
        }
        claims.push(claim);
    }
    if let Some(record) = group {
        output.insert(
            record,
            reusable_field_claims(
                &mut claims,
                read_at,
                (record, field),
                &mut known,
                live_claim,
            )?,
        );
    }
    for policy in known {
        policy?;
    }
    Ok(output)
}
