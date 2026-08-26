use crate::setup::{authenticate, DesktopProblem, SetupSecretStore};
use fasti_application::{IdentityPort, ListRecordsQuery};
use fasti_domain::{Grain, InterpretationState, OccurredAt, RecordStatus, ResolvedField};
use fasti_store::SqliteKernel;
use serde::Serialize;

/// Wire projection of [`fasti_domain::ResolvedField`]. Reuses the domain
/// enum's own `Serialize` impl for `tier` rather than re-deriving the same
/// snake_case mapping here.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ResolvedFieldView {
    tier: fasti_domain::FieldResolutionTier,
    value: Option<String>,
    source: Option<String>,
    is_stale: bool,
}

impl From<&ResolvedField> for ResolvedFieldView {
    fn from(field: &ResolvedField) -> Self {
        Self {
            tier: field.tier(),
            value: field.value().map(ToOwned::to_owned),
            source: field.source().map(ToString::to_string),
            is_stale: field.is_stale(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RecordActivityView {
    occurred_at: Option<OccurredAt>,
    interpretation_state: InterpretationState,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RecordSummary {
    record_id: String,
    /// Identity granularity, not the frontend's display `MediaKind`. A later
    /// frontend-wiring pass owns the `Grain` -> `MediaKind` projection.
    grain: Grain,
    status: RecordStatus,
    title: ResolvedFieldView,
    poster: ResolvedFieldView,
    latest_activity: Option<RecordActivityView>,
}

fn require_access(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
) -> Result<fasti_application::RequestAccessContext, DesktopProblem> {
    authenticate(kernel, store)?.ok_or_else(DesktopProblem::not_authenticated)
}

pub(crate) fn list_records(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
) -> Result<Vec<RecordSummary>, DesktopProblem> {
    let access = require_access(kernel, store)?;
    let correlation_id = fasti_domain::RequestCorrelationId::new_v7();
    let summaries = kernel
        .list_records(ListRecordsQuery::new(correlation_id, access))
        .map_err(|problem| DesktopProblem::application(&problem))?;
    Ok(summaries
        .into_iter()
        .map(|summary| RecordSummary {
            record_id: summary.record_id().to_string(),
            grain: summary.grain(),
            status: summary.status(),
            title: summary.title().into(),
            poster: summary.poster().into(),
            latest_activity: summary
                .latest_activity()
                .map(|activity| RecordActivityView {
                    occurred_at: activity.occurred_at().cloned(),
                    interpretation_state: activity.interpretation_state(),
                }),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::complete_setup;
    use crate::setup::test_support::{new_kernel, MemoryStore};

    #[test]
    fn list_records_refuses_before_setup_completes() {
        let (_root, kernel) = new_kernel();
        let store = MemoryStore::default();

        assert!(matches!(
            list_records(&kernel, &store),
            Err(problem) if problem.code() == "not_authenticated"
        ));
    }

    #[test]
    fn list_records_is_honestly_empty_on_a_fresh_node() {
        let (_root, kernel) = new_kernel();
        let store = MemoryStore::default();
        complete_setup(&kernel, &store).expect("complete setup");

        let records = list_records(&kernel, &store).expect("list records");
        assert!(records.is_empty());
    }
}
