//! Atomic Search actions. Identity and metadata retain their existing writers.
use crate::identity::{
    attach_identifier_tx, insert_record, matching_record_ids, register_namespace_tx,
};
use crate::kernel::{authorize_application_transaction, map_sql, now, parse_timestamp, timestamp};
use crate::search::{provider_snapshot, read_search_candidate};
use crate::SqliteKernel;
use fasti_application::*;
use fasti_domain::{
    FieldClaimStatus, RequestCorrelationId, ORIGINAL_TITLE_FIELD_KEY, OVERVIEW_FIELD_KEY,
    POSTER_FIELD_KEY, RELEASE_YEAR_FIELD_KEY, TITLE_FIELD_KEY,
};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};

const CAPABILITY: CapabilityKey = CapabilityKey::AttachIdentifier;

fn problem(code: ProblemCode, id: RequestCorrelationId) -> Box<FastiProblem> {
    Box::new(FastiProblem::from_code(code, CAPABILITY, id))
}

// Only the read owner's observed errors are transferable to this action.
fn search_problem(error: &FastiProblem, id: RequestCorrelationId) -> Box<FastiProblem> {
    let code = match error.code() {
        code @ (ProblemCode::AuthenticationFailed
        | ProblemCode::BrowserSessionExpired
        | ProblemCode::BrowserSessionRevoked
        | ProblemCode::CapabilityUnavailable
        | ProblemCode::Forbidden
        | ProblemCode::IntegrityFailed
        | ProblemCode::SessionPolicyChanged
        | ProblemCode::StorageUnavailable
        | ProblemCode::ValidationFailed) => code,
        _ => ProblemCode::IntegrityFailed,
    };
    problem(code, id)
}

fn subject(access: AuthorizedApplicationAccess) -> Option<fasti_domain::AuthSubjectId> {
    match access.actor() {
        AuthorizedActor::BrowserSession {
            auth_subject_id, ..
        } => Some(auth_subject_id),
        AuthorizedActor::Credential { .. } => None,
    }
}

/// Shared by replay and archive import. Canonical bounded bytes are mandatory.
pub(crate) fn decode_receipt(
    json: &str,
    id: RequestCorrelationId,
) -> ApplicationResult<SearchCandidateActionReceipt> {
    if json.len() > MAX_SEARCH_ACTION_RECEIPT_BYTES {
        return Err(problem(ProblemCode::IntegrityFailed, id));
    }
    let receipt: SearchCandidateActionReceipt =
        serde_json::from_str(json).map_err(|_| problem(ProblemCode::IntegrityFailed, id))?;
    if serde_json::to_string(&receipt).map_err(|_| problem(ProblemCode::IntegrityFailed, id))?
        != json
    {
        return Err(problem(ProblemCode::IntegrityFailed, id));
    }
    let mapping = provider_identity_mapping_for_grain(&receipt.provider, receipt.grain)
        .ok_or_else(|| problem(ProblemCode::IntegrityFailed, id))?;
    let provenance = &receipt.provenance;
    let identifier = mapping
        .identifier(provenance.source_identifier().unwrap_or_default())
        .map_err(|_| problem(ProblemCode::IntegrityFailed, id))?;
    let action_valid = match (receipt.action, receipt.disposition) {
        (
            SearchRecordAction::Create,
            SearchRecordActionDisposition::Created | SearchRecordActionDisposition::Reused,
        ) => true,
        (
            SearchRecordAction::Attach(target),
            SearchRecordActionDisposition::Attached
            | SearchRecordActionDisposition::AlreadyAttached,
        ) => target == receipt.record_id,
        _ => false,
    };
    let freshness_valid = match receipt.evidence_mode {
        SearchCandidateEvidenceMode::Cached => {
            provenance.evidence_digest() == Some(&receipt.search_response_digest)
                && match (receipt.initial_status, receipt.expires_at) {
                    (FieldClaimStatus::Stale, None) => true,
                    (FieldClaimStatus::Fresh, Some(expiry)) => {
                        expiry > receipt.fetched_at
                            && receipt
                                .fetched_at
                                .checked_add_signed(chrono::Duration::seconds(SEARCH_FRESH_SECONDS))
                                .is_some_and(|cap| expiry <= cap)
                    }
                    _ => false,
                }
        }
        SearchCandidateEvidenceMode::Refetch => {
            match (receipt.initial_status, receipt.expires_at) {
                (FieldClaimStatus::Stale, None) => true,
                (FieldClaimStatus::Fresh, Some(expiry)) => {
                    expiry > receipt.fetched_at
                        && receipt
                            .fetched_at
                            .checked_add_signed(chrono::Duration::seconds(
                                fasti_domain::METADATA_FRESH_SECONDS,
                            ))
                            .is_some_and(|cap| expiry <= cap)
                }
                _ => false,
            }
        }
    };
    if !action_valid
        || !freshness_valid
        || !provenance.is_complete()
        || provenance.provider_id().map(|provider| provider.as_str())
            != Some(receipt.provider.as_str())
        || provenance.source_namespace().as_str() != identifier.namespace()
        || provenance.source_identifier() != Some(identifier.value())
        || provenance.region().is_some()
        || receipt.fetched_at > receipt.committed_at
        || (receipt.evidence_mode == SearchCandidateEvidenceMode::Refetch
            && provenance.locale()
                != provider_metadata_response_locale(&receipt.provider, provenance.locale())
                    .as_ref())
    {
        return Err(problem(ProblemCode::IntegrityFailed, id));
    }
    Ok(receipt)
}

pub(crate) fn decode_provider_identifier_receipt(
    json: &str,
    id: RequestCorrelationId,
) -> ApplicationResult<ProviderIdentifierActionReceipt> {
    if json.len() > MAX_SEARCH_ACTION_RECEIPT_BYTES {
        return Err(problem(ProblemCode::IntegrityFailed, id));
    }
    let receipt: ProviderIdentifierActionReceipt =
        serde_json::from_str(json).map_err(|_| problem(ProblemCode::IntegrityFailed, id))?;
    if serde_json::to_string(&receipt).map_err(|_| problem(ProblemCode::IntegrityFailed, id))?
        != json
    {
        return Err(problem(ProblemCode::IntegrityFailed, id));
    }
    let mapping = provider_identity_mapping_for_grain(&receipt.provider, receipt.grain)
        .ok_or_else(|| problem(ProblemCode::IntegrityFailed, id))?;
    mapping
        .identifier(receipt.provider_record_id.clone())
        .map_err(|_| problem(ProblemCode::IntegrityFailed, id))?;
    let action_valid = match (receipt.action, receipt.disposition) {
        (
            SearchRecordAction::Create,
            SearchRecordActionDisposition::Created | SearchRecordActionDisposition::Reused,
        ) => true,
        (
            SearchRecordAction::Attach(target),
            SearchRecordActionDisposition::Attached
            | SearchRecordActionDisposition::AlreadyAttached,
        ) => target == receipt.record_id,
        _ => false,
    };
    if !action_valid
        || receipt.origin != ProviderIdentifierActionOrigin::UserSelectedProviderIdentifier
    {
        return Err(problem(ProblemCode::IntegrityFailed, id));
    }
    Ok(receipt)
}

fn ensure_receipt_capacity(
    tx: &Transaction<'_>,
    workspace_id: fasti_domain::WorkspaceId,
    limits: SearchActionReceiptLimits,
    proposed_json_bytes: Option<usize>,
    id: RequestCorrelationId,
) -> ApplicationResult<()> {
    let (rows, bytes) = map_sql(
        tx.query_row(
            "SELECT COUNT(*), COALESCE(SUM(length(CAST(receipt_json AS BLOB))), 0) FROM search_action_receipts WHERE workspace_id = ?1",
            [workspace_id.to_string()],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        ),
        CAPABILITY,
        id,
    )?;
    let rows = u64::try_from(rows).map_err(|_| problem(ProblemCode::IntegrityFailed, id))?;
    let bytes = u64::try_from(bytes).map_err(|_| problem(ProblemCode::IntegrityFailed, id))?;
    let proposed = u64::try_from(proposed_json_bytes.unwrap_or_default())
        .map_err(|_| problem(ProblemCode::CapacityExceeded, id))?;
    if rows >= limits.max_rows()
        || bytes
            .checked_add(proposed)
            .is_none_or(|total| total > limits.max_receipt_json_bytes())
        || (proposed_json_bytes.is_none() && bytes >= limits.max_receipt_json_bytes())
    {
        return Err(problem(ProblemCode::CapacityExceeded, id));
    }
    Ok(())
}

fn replay(
    tx: &Transaction<'_>,
    access: AuthorizedApplicationAccess,
    command: &SearchCandidateActionCommand,
) -> ApplicationResult<Option<SearchCandidateActionReceipt>> {
    let id = command.request.correlation_id;
    let row = map_sql(tx.query_row(
        "SELECT profile_id, actor_client_id, actor_subject_id, semantic_digest, record_id, receipt_json FROM search_action_receipts WHERE workspace_id = ?1 AND operation_id = ?2",
        params![access.workspace_id().to_string(), command.operation_id.to_string()],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, String>(5)?)),
    ).optional(), CAPABILITY, id)?;
    let Some((profile, client, actor_subject, digest, record, json)) = row else {
        return Ok(None);
    };
    if profile != access.profile_id().to_string()
        || client != access.attribution_client_id().to_string()
        || actor_subject != subject(access).map(|value| value.to_string())
        || digest != command.semantic_digest().as_str()
    {
        return Err(problem(ProblemCode::IdempotencyConflict, id));
    }
    let receipt = decode_receipt(&json, id)?;
    if receipt.workspace_id != access.workspace_id()
        || receipt.profile_id != access.profile_id()
        || receipt.actor_client_id != access.attribution_client_id()
        || receipt.actor_subject_id != subject(access)
        || receipt.operation_id != command.operation_id
        || receipt.record_id.to_string() != record
        || receipt.semantic_digest() != command.semantic_digest()
    {
        return Err(problem(ProblemCode::IntegrityFailed, id));
    }
    Ok(Some(receipt))
}

fn prepare_tx(
    tx: &Transaction<'_>,
    command: &SearchCandidateActionCommand,
    limits: SearchActionReceiptLimits,
) -> ApplicationResult<SearchCandidateActionPreparation> {
    let request = &command.request;
    let id = request.correlation_id;
    let access = authorize_application_transaction(tx, CAPABILITY, &request.access, id)?;
    if let Some(receipt) = replay(tx, access, command)? {
        return Ok(SearchCandidateActionPreparation::Replay(Box::new(receipt)));
    }
    let search =
        authorize_application_transaction(tx, CapabilityKey::SearchMetadata, &request.access, id)
            .map_err(|error| search_problem(&error, id))?;
    ensure_receipt_capacity(tx, access.workspace_id(), limits, None, id)?;
    let candidate = read_search_candidate(tx, request, search)
        .map_err(|error| search_problem(&error, id))?
        .ok_or_else(|| problem(ProblemCode::ValidationFailed, id))?;
    match command.evidence_mode {
        SearchCandidateEvidenceMode::Cached => {
            if !candidate.payload_is_reusable(now()) {
                return Err(problem(ProblemCode::ValidationFailed, id));
            }
            Ok(SearchCandidateActionPreparation::Cached(candidate))
        }
        SearchCandidateEvidenceMode::Refetch => {
            let (provider_state, provider_authority_fingerprint) = provider_snapshot(
                tx,
                access.workspace_id(),
                candidate.context.provider(),
                "metadata.read",
                &request.outbound_policy,
                id,
            )
            .map_err(|error| search_problem(&error, id))?;
            Ok(SearchCandidateActionPreparation::Refetch(
                PreparedSearchCandidateDetails {
                    candidate,
                    provider_state,
                    provider_authority_fingerprint,
                },
            ))
        }
    }
}

pub(crate) fn prepare(
    kernel: &SqliteKernel,
    command: &SearchCandidateActionCommand,
) -> ApplicationResult<SearchCandidateActionPreparation> {
    let id = command.request.correlation_id;
    let mut connection = kernel.lock_connection(CAPABILITY, id)?;
    let tx = map_sql(
        connection.transaction_with_behavior(TransactionBehavior::Immediate),
        CAPABILITY,
        id,
    )?;
    match prepare_tx(&tx, command, kernel.inner.search_action_receipt_limits) {
        Ok(result) => {
            map_sql(tx.commit(), CAPABILITY, id)?;
            Ok(result)
        }
        Err(error) => {
            map_sql(tx.rollback(), CAPABILITY, id)?;
            Err(error)
        }
    }
}

fn replay_provider_identifier(
    tx: &Transaction<'_>,
    access: AuthorizedApplicationAccess,
    command: &ProviderIdentifierActionCommand,
) -> ApplicationResult<Option<ProviderIdentifierActionReceipt>> {
    let id = command.correlation_id;
    let row = map_sql(
        tx.query_row(
            "SELECT profile_id, actor_client_id, actor_subject_id, semantic_digest, record_id, receipt_json FROM search_action_receipts WHERE workspace_id = ?1 AND operation_id = ?2",
            params![access.workspace_id().to_string(), command.operation_id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?)),
        )
        .optional(),
        CAPABILITY,
        id,
    )?;
    let Some((profile, client, actor_subject, digest, record, json)) = row else {
        return Ok(None);
    };
    if profile != access.profile_id().to_string()
        || client != access.attribution_client_id().to_string()
        || actor_subject != subject(access).map(|value| value.to_string())
        || digest != command.semantic_digest().as_str()
    {
        return Err(problem(ProblemCode::IdempotencyConflict, id));
    }
    let receipt = decode_provider_identifier_receipt(&json, id)?;
    if receipt.workspace_id != access.workspace_id()
        || receipt.profile_id != access.profile_id()
        || receipt.actor_client_id != access.attribution_client_id()
        || receipt.actor_subject_id != subject(access)
        || receipt.operation_id != command.operation_id
        || receipt.record_id.to_string() != record
        || receipt.semantic_digest() != command.semantic_digest()
    {
        return Err(problem(ProblemCode::IntegrityFailed, id));
    }
    Ok(Some(receipt))
}

fn prepare_provider_identifier_tx(
    tx: &Transaction<'_>,
    command: &ProviderIdentifierActionCommand,
    limits: SearchActionReceiptLimits,
) -> ApplicationResult<ProviderIdentifierActionPreparation> {
    let id = command.correlation_id;
    let access = authorize_application_transaction(tx, CAPABILITY, &command.access, id)?;
    if let Some(receipt) = replay_provider_identifier(tx, access, command)? {
        return Ok(ProviderIdentifierActionPreparation::Replay(Box::new(
            receipt,
        )));
    }
    authorize_application_transaction(tx, CapabilityKey::SearchMetadata, &command.access, id)
        .map_err(|error| search_problem(&error, id))?;
    ensure_receipt_capacity(tx, access.workspace_id(), limits, None, id)?;
    let mapping = provider_identity_mapping_for_grain(command.provider.as_str(), command.grain)
        .ok_or_else(|| problem(ProblemCode::InvalidIdentifier, id))?;
    mapping
        .identifier(command.provider_record_id.clone())
        .map_err(|_| problem(ProblemCode::InvalidIdentifier, id))?;
    let (provider_state, provider_authority_fingerprint) = provider_snapshot(
        tx,
        access.workspace_id(),
        command.provider.as_str(),
        "metadata.read",
        &command.outbound_policy,
        id,
    )
    .map_err(|error| search_problem(&error, id))?;
    Ok(ProviderIdentifierActionPreparation::Refetch {
        provider_state,
        provider_authority_fingerprint,
    })
}

pub(crate) fn prepare_provider_identifier(
    kernel: &SqliteKernel,
    command: &ProviderIdentifierActionCommand,
) -> ApplicationResult<ProviderIdentifierActionPreparation> {
    let id = command.correlation_id;
    let mut connection = kernel.lock_connection(CAPABILITY, id)?;
    let tx = map_sql(
        connection.transaction_with_behavior(TransactionBehavior::Immediate),
        CAPABILITY,
        id,
    )?;
    match prepare_provider_identifier_tx(&tx, command, kernel.inner.search_action_receipt_limits) {
        Ok(result) => {
            map_sql(tx.commit(), CAPABILITY, id)?;
            Ok(result)
        }
        Err(error) => {
            map_sql(tx.rollback(), CAPABILITY, id)?;
            Err(error)
        }
    }
}

pub(crate) fn commit_provider_identifier(
    kernel: &SqliteKernel,
    command: &ProviderIdentifierActionCommand,
    prepared: &ProviderIdentifierActionPreparation,
    confirmed_identifier: &fasti_domain::ExternalIdentifierClaim,
) -> ApplicationResult<ProviderIdentifierActionReceipt> {
    let id = command.correlation_id;
    let mut connection = kernel.lock_connection(CAPABILITY, id)?;
    let tx = map_sql(
        connection.transaction_with_behavior(TransactionBehavior::Immediate),
        CAPABILITY,
        id,
    )?;
    let current = match prepare_provider_identifier_tx(
        &tx,
        command,
        kernel.inner.search_action_receipt_limits,
    ) {
        Ok(current) => current,
        Err(error) => {
            map_sql(tx.rollback(), CAPABILITY, id)?;
            return Err(error);
        }
    };
    if let ProviderIdentifierActionPreparation::Replay(receipt) = current {
        map_sql(tx.commit(), CAPABILITY, id)?;
        return Ok(*receipt);
    }
    let authority_matches = matches!(
        (&current, prepared),
        (
            ProviderIdentifierActionPreparation::Refetch {
                provider_authority_fingerprint: current,
                ..
            },
            ProviderIdentifierActionPreparation::Refetch {
                provider_authority_fingerprint: prepared,
                ..
            }
        ) if current == prepared
    );
    if !authority_matches {
        return Err(problem(ProblemCode::Forbidden, id));
    }
    let mapping = provider_identity_mapping_for_grain(command.provider.as_str(), command.grain)
        .ok_or_else(|| problem(ProblemCode::InvalidIdentifier, id))?;
    let identifier = mapping
        .identifier(command.provider_record_id.clone())
        .map_err(|_| problem(ProblemCode::InvalidIdentifier, id))?;
    if &identifier != confirmed_identifier {
        return Err(problem(ProblemCode::ValidationFailed, id));
    }
    let access = authorize_application_transaction(&tx, CAPABILITY, &command.access, id)?;
    register_namespace_tx(
        &tx,
        access.workspace_id(),
        &mapping
            .namespace_definition()
            .map_err(|_| problem(ProblemCode::InvalidIdentifier, id))?,
        CAPABILITY,
        id,
    )?;
    let (record_id, mut disposition) = match command.action {
        SearchRecordAction::Create => {
            let existing = matching_record_ids(
                &tx,
                access.workspace_id(),
                std::slice::from_ref(&identifier),
                CAPABILITY,
                id,
            )?;
            if let Some(record_id) = existing.first() {
                (*record_id, SearchRecordActionDisposition::Reused)
            } else {
                (
                    insert_record(&tx, access.workspace_id(), command.grain, CAPABILITY, id)?,
                    SearchRecordActionDisposition::Created,
                )
            }
        }
        SearchRecordAction::Attach(record) => (record, SearchRecordActionDisposition::Attached),
    };
    let attached = attach_identifier_tx(
        &tx,
        access.workspace_id(),
        record_id,
        &identifier,
        CAPABILITY,
        id,
    )?;
    if matches!(command.action, SearchRecordAction::Attach(_)) && !attached.created() {
        disposition = SearchRecordActionDisposition::AlreadyAttached;
    }
    let receipt = ProviderIdentifierActionReceipt {
        workspace_id: access.workspace_id(),
        profile_id: access.profile_id(),
        actor_client_id: access.attribution_client_id(),
        actor_subject_id: subject(access),
        operation_id: command.operation_id,
        provider: command.provider.as_str().to_owned(),
        provider_record_id: command.provider_record_id.clone(),
        grain: command.grain,
        action: command.action,
        origin: ProviderIdentifierActionOrigin::UserSelectedProviderIdentifier,
        record_id,
        disposition,
        committed_at: now(),
    };
    let json =
        serde_json::to_string(&receipt).map_err(|_| problem(ProblemCode::IntegrityFailed, id))?;
    let receipt = decode_provider_identifier_receipt(&json, id)?;
    if let Err(error) = ensure_receipt_capacity(
        &tx,
        access.workspace_id(),
        kernel.inner.search_action_receipt_limits,
        Some(json.len()),
        id,
    ) {
        map_sql(tx.rollback(), CAPABILITY, id)?;
        return Err(error);
    }
    map_sql(
        tx.execute(
            "INSERT INTO search_action_receipts(workspace_id,operation_id,profile_id,actor_client_id,actor_subject_id,record_id,semantic_digest,receipt_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![receipt.workspace_id.to_string(), receipt.operation_id.to_string(), receipt.profile_id.to_string(),
                receipt.actor_client_id.to_string(), receipt.actor_subject_id.map(|value| value.to_string()),
                receipt.record_id.to_string(), command.semantic_digest().as_str(), json],
        ),
        CAPABILITY,
        id,
    )?;
    map_sql(tx.commit(), CAPABILITY, id)?;
    Ok(receipt)
}

pub(crate) fn commit(
    kernel: &SqliteKernel,
    command: &SearchCandidateActionCommand,
    prepared: &SearchCandidateActionPreparation,
    refetched_fields: Option<(
        &[ProviderMetadataField],
        &fasti_application::ProviderResponseCachePolicy,
    )>,
) -> ApplicationResult<SearchCandidateActionReceipt> {
    let request = &command.request;
    let id = request.correlation_id;
    let mut connection = kernel.lock_connection(CAPABILITY, id)?;
    let tx = map_sql(
        connection.transaction_with_behavior(TransactionBehavior::Immediate),
        CAPABILITY,
        id,
    )?;
    let current = match prepare_tx(&tx, command, kernel.inner.search_action_receipt_limits) {
        Ok(current) => current,
        Err(error) => {
            map_sql(tx.rollback(), CAPABILITY, id)?;
            return Err(error);
        }
    };
    if let SearchCandidateActionPreparation::Replay(receipt) = current {
        map_sql(tx.commit(), CAPABILITY, id)?;
        return Ok(*receipt);
    }
    let cached_fields;
    let (snapshot, fields, response_policy) = match (&current, prepared) {
        (
            SearchCandidateActionPreparation::Cached(current),
            SearchCandidateActionPreparation::Cached(original),
        ) if current == original && refetched_fields.is_none() => {
            cached_fields = current
                .metadata_fields()
                .map_err(|_| problem(ProblemCode::IntegrityFailed, id))?;
            (current, cached_fields.as_slice(), &current.response_policy)
        }
        (
            SearchCandidateActionPreparation::Refetch(current),
            SearchCandidateActionPreparation::Refetch(original),
        ) if current.candidate == original.candidate
            && current.provider_authority_fingerprint
                == original.provider_authority_fingerprint =>
        {
            let (fields, policy) =
                refetched_fields.ok_or_else(|| problem(ProblemCode::ValidationFailed, id))?;
            (&current.candidate, fields, policy)
        }
        _ => return Err(problem(ProblemCode::Forbidden, id)),
    };
    let first = fields
        .first()
        .ok_or_else(|| problem(ProblemCode::ValidationFailed, id))?
        .claim();
    let identifier = snapshot.receipt.candidate().identifier();
    let expected_locale = match command.evidence_mode {
        SearchCandidateEvidenceMode::Cached => snapshot.context.locale().cloned(),
        SearchCandidateEvidenceMode::Refetch => {
            provider_metadata_response_locale(request.provider.as_str(), snapshot.context.locale())
        }
    };
    if fields.len() > MAX_PROVIDER_METADATA_FIELDS
        || !fields
            .iter()
            .any(|field| field.field_key().as_str() == TITLE_FIELD_KEY)
        || first
            .provenance()
            .provider_id()
            .map(|provider| provider.as_str())
            != Some(request.provider.as_str())
        || first.provenance().source_namespace().as_str() != identifier.namespace()
        || first.provenance().source_identifier() != Some(identifier.value())
        || first.provenance().locale() != expected_locale.as_ref()
        || first.provenance().region().is_some()
        || first.fetched_at() < snapshot.receipt.lifetime().created_at()
        || fields.iter().any(|field| {
            metadata_field_group(field.field_key()).is_none()
                || field.claim().record_id().is_some()
                || field.claim().field_key().is_some()
                || field.claim().provenance() != first.provenance()
                || field.claim().fetched_at() != first.fetched_at()
                || field.claim().expires_at() != first.expires_at()
                || field.claim().initial_status() != first.initial_status()
        })
    {
        return Err(problem(ProblemCode::ValidationFailed, id));
    }
    // Reuse the same public-field bounds as the provider adapter. A typed claim
    // alone permits wider field values than a normalized provider candidate.
    let mut normalized = SearchCandidateData {
        provider: request.provider.as_str().to_owned(),
        provider_id: identifier.value().to_owned(),
        kind: snapshot.receipt.candidate().data().kind.clone(),
        title: String::new(),
        original_title: None,
        release_year: None,
        authors: Vec::new(),
        image_url: None,
        overview: None,
    };
    for field in fields {
        let value = field.claim().value();
        match field.field_key().as_str() {
            TITLE_FIELD_KEY => normalized.title = value.to_owned(),
            ORIGINAL_TITLE_FIELD_KEY => normalized.original_title = Some(value.to_owned()),
            OVERVIEW_FIELD_KEY => normalized.overview = Some(value.to_owned()),
            POSTER_FIELD_KEY => normalized.image_url = Some(value.to_owned()),
            RELEASE_YEAR_FIELD_KEY => {
                normalized.release_year = Some(
                    value
                        .parse()
                        .map_err(|_| problem(ProblemCode::ValidationFailed, id))?,
                )
            }
            _ => return Err(problem(ProblemCode::ValidationFailed, id)),
        }
    }
    SearchCandidate::try_new(normalized).map_err(|_| problem(ProblemCode::ValidationFailed, id))?;
    let access = authorize_application_transaction(&tx, CAPABILITY, &request.access, id)?;
    crate::metadata::validate_provider_fields(
        identifier,
        None,
        fields,
        response_policy,
        CAPABILITY,
        id,
    )?;
    let mapping = provider_identity_mapping_for_grain(request.provider.as_str(), request.grain)
        .ok_or_else(|| problem(ProblemCode::InvalidIdentifier, id))?;
    register_namespace_tx(
        &tx,
        access.workspace_id(),
        &mapping
            .namespace_definition()
            .map_err(|_| problem(ProblemCode::InvalidIdentifier, id))?,
        CAPABILITY,
        id,
    )?;
    let (record_id, mut disposition) = match command.action {
        SearchRecordAction::Create => {
            let existing = matching_record_ids(
                &tx,
                access.workspace_id(),
                std::slice::from_ref(identifier),
                CAPABILITY,
                id,
            )?;
            if let Some(record_id) = existing.first() {
                (*record_id, SearchRecordActionDisposition::Reused)
            } else {
                (
                    insert_record(&tx, access.workspace_id(), request.grain, CAPABILITY, id)?,
                    SearchRecordActionDisposition::Created,
                )
            }
        }
        SearchRecordAction::Attach(record) => (record, SearchRecordActionDisposition::Attached),
    };
    let attached = attach_identifier_tx(
        &tx,
        access.workspace_id(),
        record_id,
        identifier,
        CAPABILITY,
        id,
    )?;
    if matches!(command.action, SearchRecordAction::Attach(_)) && !attached.created() {
        disposition = SearchRecordActionDisposition::AlreadyAttached;
    }
    crate::metadata::write_provider_fields(
        &tx,
        access.workspace_id(),
        record_id,
        identifier,
        fields,
        CAPABILITY,
        id,
        response_policy,
    )?;
    let receipt = SearchCandidateActionReceipt {
        workspace_id: access.workspace_id(),
        profile_id: access.profile_id(),
        actor_client_id: access.attribution_client_id(),
        actor_subject_id: subject(access),
        operation_id: command.operation_id,
        candidate_receipt_id: request.candidate_receipt_id,
        provider: request.provider.as_str().to_owned(),
        grain: request.grain,
        action: command.action,
        evidence_mode: command.evidence_mode,
        record_id,
        disposition,
        search_context_digest: snapshot.context.digest(),
        search_response_digest: snapshot.receipt.response_digest().clone(),
        provenance: first.provenance().clone(),
        fetched_at: parse_timestamp(&timestamp(first.fetched_at()), CAPABILITY, id)?,
        expires_at: first
            .expires_at()
            .map(|value| parse_timestamp(&timestamp(value), CAPABILITY, id))
            .transpose()?,
        initial_status: first.initial_status(),
        committed_at: now(),
    };
    let json =
        serde_json::to_string(&receipt).map_err(|_| problem(ProblemCode::IntegrityFailed, id))?;
    let receipt = decode_receipt(&json, id)?;
    if let Err(error) = ensure_receipt_capacity(
        &tx,
        access.workspace_id(),
        kernel.inner.search_action_receipt_limits,
        Some(json.len()),
        id,
    ) {
        map_sql(tx.rollback(), CAPABILITY, id)?;
        return Err(error);
    }
    map_sql(tx.execute("INSERT INTO search_action_receipts(workspace_id,operation_id,profile_id,actor_client_id,actor_subject_id,record_id,semantic_digest,receipt_json) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![receipt.workspace_id.to_string(), receipt.operation_id.to_string(), receipt.profile_id.to_string(),
            receipt.actor_client_id.to_string(), receipt.actor_subject_id.map(|value| value.to_string()),
            receipt.record_id.to_string(), command.semantic_digest().as_str(), json]), CAPABILITY, id)?;
    map_sql(tx.commit(), CAPABILITY, id)?;
    Ok(receipt)
}
