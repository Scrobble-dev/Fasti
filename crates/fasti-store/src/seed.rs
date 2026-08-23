use crate::identity::{attach_identifier_tx, insert_record, matching_record_ids};
use crate::kernel::{authorize_transaction, map_sql, SqliteKernel};
use fasti_application::{
    ApplicationResult, ApplyIdentitySeedCommand, ApplyIdentitySeedOutcome, CapabilityKey,
    FastiProblem, IdentitySeedDisposition, IdentitySeedEntryOutcome, IdentitySeedPort, ProblemCode,
    MAX_IDENTITY_CLAIMS, MAX_IDENTITY_SEED_ENTRIES, MAX_IDENTITY_SEED_KEY_BYTES,
    MAX_IDENTITY_SEED_VERSION_BYTES,
};
use rusqlite::TransactionBehavior;
use std::collections::BTreeSet;

impl IdentitySeedPort for SqliteKernel {
    fn apply_identity_seed(
        &self,
        command: ApplyIdentitySeedCommand,
    ) -> ApplicationResult<ApplyIdentitySeedOutcome> {
        let capability = CapabilityKey::CreateRecord;
        let correlation_id = command.correlation_id();
        if command.manifest().version().trim().is_empty()
            || command.manifest().version().len() > MAX_IDENTITY_SEED_VERSION_BYTES
            || command.manifest().entries().len() > MAX_IDENTITY_SEED_ENTRIES
        {
            return Err(Box::new(FastiProblem::from_code(
                ProblemCode::ValidationFailed,
                capability,
                correlation_id,
            )));
        }
        let mut keys = BTreeSet::new();
        for entry in command.manifest().entries() {
            if entry.key().trim().is_empty()
                || entry.key().len() > MAX_IDENTITY_SEED_KEY_BYTES
                || !keys.insert(entry.key())
                || entry.identifiers().is_empty()
                || entry.identifiers().len() > MAX_IDENTITY_CLAIMS
                || entry
                    .identifiers()
                    .iter()
                    .any(|claim| claim.grain() != entry.grain())
            {
                return Err(Box::new(FastiProblem::from_code(
                    ProblemCode::ValidationFailed,
                    capability,
                    correlation_id,
                )));
            }
        }

        let mut connection = self.lock_connection(capability, correlation_id)?;
        let transaction = map_sql(
            connection.transaction_with_behavior(TransactionBehavior::Immediate),
            capability,
            correlation_id,
        )?;
        authorize_transaction(&transaction, capability, command.access(), correlation_id)?;
        let mut outcomes = Vec::with_capacity(command.manifest().entries().len());
        for entry in command.manifest().entries() {
            let matches = matching_record_ids(
                &transaction,
                command.access().workspace_id(),
                entry.identifiers(),
                capability,
                correlation_id,
            )?;
            match matches.as_slice() {
                [record_id] => {
                    if !command.dry_run() {
                        for claim in entry.identifiers() {
                            attach_identifier_tx(
                                &transaction,
                                command.access().workspace_id(),
                                *record_id,
                                claim,
                                capability,
                                correlation_id,
                            )?;
                        }
                    }
                    outcomes.push(IdentitySeedEntryOutcome::new(
                        entry.key(),
                        IdentitySeedDisposition::Reused,
                        Some(*record_id),
                    ));
                }
                [] if command.dry_run() => {
                    outcomes.push(IdentitySeedEntryOutcome::new(
                        entry.key(),
                        IdentitySeedDisposition::WouldCreate,
                        None,
                    ));
                }
                [] => {
                    let record_id = insert_record(
                        &transaction,
                        command.access().workspace_id(),
                        entry.grain(),
                        capability,
                        correlation_id,
                    )?;
                    for claim in entry.identifiers() {
                        attach_identifier_tx(
                            &transaction,
                            command.access().workspace_id(),
                            record_id,
                            claim,
                            capability,
                            correlation_id,
                        )?;
                    }
                    outcomes.push(IdentitySeedEntryOutcome::new(
                        entry.key(),
                        IdentitySeedDisposition::Created,
                        Some(record_id),
                    ));
                }
                _ => {
                    outcomes.push(IdentitySeedEntryOutcome::new(
                        entry.key(),
                        IdentitySeedDisposition::Conflict,
                        None,
                    ));
                }
            }
        }
        map_sql(transaction.commit(), capability, correlation_id)?;
        Ok(ApplyIdentitySeedOutcome::new(
            command.manifest().version(),
            command.dry_run(),
            outcomes,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestNode;
    use fasti_application::{IdentitySeedEntry, IdentitySeedManifest};
    use fasti_domain::{ExternalIdentifierClaim, Grain, RequestCorrelationId};

    #[test]
    fn identity_seed_bounds_version_and_identifier_fanout() {
        let node = TestNode::new();
        let identifiers = (0..=MAX_IDENTITY_CLAIMS)
            .map(|index| {
                ExternalIdentifierClaim::try_new("tmdb", Grain::Release, index.to_string())
                    .expect("valid identifier")
            })
            .collect();
        let manifest = IdentitySeedManifest::new(
            "v1",
            vec![IdentitySeedEntry::new(
                "entry-1",
                Grain::Release,
                identifiers,
            )],
        );
        let command = ApplyIdentitySeedCommand::new(
            RequestCorrelationId::new_v7(),
            node.access,
            manifest,
            false,
        );
        let error = node
            .kernel
            .apply_identity_seed(command)
            .expect_err("identifier limit");
        assert_eq!(error.code(), ProblemCode::ValidationFailed);

        let manifest =
            IdentitySeedManifest::new("x".repeat(MAX_IDENTITY_SEED_VERSION_BYTES + 1), Vec::new());
        let command = ApplyIdentitySeedCommand::new(
            RequestCorrelationId::new_v7(),
            node.access,
            manifest,
            true,
        );
        let error = node
            .kernel
            .apply_identity_seed(command)
            .expect_err("version limit");
        assert_eq!(error.code(), ProblemCode::ValidationFailed);
    }
}
