use crate::setup::{authenticate, DesktopProblem, SetupSecretStore};
use fasti_application::{ResolveReviewCommand, ReviewPort, ReviewQuery, ReviewResolutionTarget};
use fasti_domain::{ExternalIdentifierClaim, Grain, RecordId, RequestCorrelationId, ReviewItemId};
use fasti_store::SqliteKernel;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ReviewItem {
    review_item_id: String,
    observation_id: String,
    current_interpretation_id: String,
    status: &'static str,
    candidate_record_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub(crate) enum ReviewResolutionTargetInput {
    Existing(String),
    New(String),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExternalIdentifierClaimInput {
    namespace: String,
    grain: String,
    value: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResolveReviewInput {
    review_item_id: String,
    target: ReviewResolutionTargetInput,
    identifiers: Vec<ExternalIdentifierClaimInput>,
}

fn require_access(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
) -> Result<fasti_application::RequestAccessContext, DesktopProblem> {
    authenticate(kernel, store)?.ok_or_else(DesktopProblem::not_authenticated)
}

pub(crate) fn list_reviews(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
) -> Result<Vec<ReviewItem>, DesktopProblem> {
    let access = require_access(kernel, store)?;
    let correlation_id = RequestCorrelationId::new_v7();
    let items = kernel
        .inspect_reviews(ReviewQuery::new(correlation_id, access, None))
        .map_err(|problem| DesktopProblem::application(&problem))?;
    Ok(items
        .into_iter()
        .map(|item| ReviewItem {
            review_item_id: item.review_item_id().to_string(),
            observation_id: item.observation_id().to_string(),
            current_interpretation_id: item.current_interpretation_id().to_string(),
            status: match item.status() {
                fasti_domain::ReviewStatus::Open => "open",
                fasti_domain::ReviewStatus::Deferred => "deferred",
                fasti_domain::ReviewStatus::Resolved => "resolved",
            },
            candidate_record_ids: item
                .candidate_record_ids()
                .iter()
                .map(ToString::to_string)
                .collect(),
        })
        .collect())
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ResolveReviewOutcome {
    review_item_id: String,
    record_id: String,
    interpretation_id: String,
}

pub(crate) fn resolve_review(
    kernel: &SqliteKernel,
    store: &impl SetupSecretStore,
    input: ResolveReviewInput,
) -> Result<ResolveReviewOutcome, DesktopProblem> {
    let access = require_access(kernel, store)?;
    let correlation_id = RequestCorrelationId::new_v7();

    let review_item_id: ReviewItemId = input
        .review_item_id
        .parse()
        .map_err(|_| DesktopProblem::invalid_input("The review item ID is malformed."))?;

    let target = match input.target {
        ReviewResolutionTargetInput::Existing(raw) => {
            let record_id: RecordId = raw.parse().map_err(|_| {
                DesktopProblem::invalid_input("The candidate record ID is malformed.")
            })?;
            ReviewResolutionTarget::Existing(record_id)
        }
        ReviewResolutionTargetInput::New(raw) => {
            let grain: Grain = raw
                .parse()
                .map_err(|_| DesktopProblem::invalid_input("The media grain is not recognized."))?;
            ReviewResolutionTarget::New(grain)
        }
    };

    let identifiers = input
        .identifiers
        .into_iter()
        .map(|claim| {
            let grain: Grain = claim.grain.parse().map_err(|_| {
                DesktopProblem::invalid_input("An identifier's media grain is not recognized.")
            })?;
            ExternalIdentifierClaim::try_new(claim.namespace, grain, claim.value)
                .map_err(|_| DesktopProblem::invalid_input("An identifier claim is invalid."))
        })
        .collect::<Result<Vec<_>, DesktopProblem>>()?;

    let outcome = kernel
        .resolve_review(ResolveReviewCommand::new(
            correlation_id,
            access,
            review_item_id,
            target,
            identifiers,
        ))
        .map_err(|problem| DesktopProblem::application(&problem))?;

    Ok(ResolveReviewOutcome {
        review_item_id: outcome.review_item_id().to_string(),
        record_id: outcome.record_id().to_string(),
        interpretation_id: outcome.interpretation_id().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::complete_setup;
    use crate::setup::test_support::{new_kernel, MemoryStore};

    #[test]
    fn list_reviews_refuses_before_setup_completes() {
        let (_root, kernel) = new_kernel();
        let store = MemoryStore::default();

        assert!(matches!(
            list_reviews(&kernel, &store),
            Err(problem) if problem.code() == "not_authenticated"
        ));
    }

    #[test]
    fn list_reviews_is_honestly_empty_on_a_fresh_node() {
        let (_root, kernel) = new_kernel();
        let store = MemoryStore::default();
        complete_setup(&kernel, &store).expect("complete setup");

        let items = list_reviews(&kernel, &store).expect("list reviews");
        assert!(items.is_empty());
    }

    #[test]
    fn resolve_review_refuses_before_setup_completes() {
        let (_root, kernel) = new_kernel();
        let store = MemoryStore::default();

        let input = ResolveReviewInput {
            review_item_id: "rev_00000000000000000000000000".to_owned(),
            target: ReviewResolutionTargetInput::New("film".to_owned()),
            identifiers: Vec::new(),
        };

        assert!(matches!(
            resolve_review(&kernel, &store, input),
            Err(problem) if problem.code() == "not_authenticated"
        ));
    }

    #[test]
    fn resolve_review_rejects_a_malformed_review_item_id() {
        let (_root, kernel) = new_kernel();
        let store = MemoryStore::default();
        complete_setup(&kernel, &store).expect("complete setup");

        let input = ResolveReviewInput {
            review_item_id: "not-a-real-id".to_owned(),
            target: ReviewResolutionTargetInput::New("film".to_owned()),
            identifiers: Vec::new(),
        };

        assert!(matches!(
            resolve_review(&kernel, &store, input),
            Err(problem) if problem.code() == "invalid_input"
        ));
    }

    #[test]
    fn resolve_review_reports_a_well_formed_but_nonexistent_review_item() {
        // A genuine "happy path" test needs a seeded review item, which only
        // exists as a side effect of the observation -> interpretation ->
        // ambiguity-detection pipeline that fasti-store's own test suite
        // doesn't shortcut either (see review.rs's own resolution test,
        // which also uses a fresh, never-persisted ReviewItemId). This test
        // instead proves the part that's actually new here: a well-formed,
        // authenticated request reaches the real kernel and its real
        // "not found" response surfaces correctly through this layer.
        let (_root, kernel) = new_kernel();
        let store = MemoryStore::default();
        complete_setup(&kernel, &store).expect("complete setup");

        let input = ResolveReviewInput {
            review_item_id: ReviewItemId::new_v7().to_string(),
            target: ReviewResolutionTargetInput::New("film".to_owned()),
            identifiers: Vec::new(),
        };

        assert!(matches!(
            resolve_review(&kernel, &store, input),
            Err(problem) if problem.code() == "review_not_found"
        ));
    }
}
