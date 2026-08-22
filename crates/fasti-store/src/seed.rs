use crate::identity::{attach_identifier_tx, insert_record, matching_record_ids};
use crate::kernel::{authorize_transaction, map_sql, SqliteKernel};
use fasti_application::{
    ApplyIdentitySeedCommand, ApplyIdentitySeedOutcome, ApplicationResult, CapabilityKey,
    FastiProblem, IdentitySeedDisposition, IdentitySeedEntryOutcome, IdentitySeedPort, ProblemCode,
};
use rusqlite::{TransactionBehavior};
use std::collections::BTreeSet;

const MAX_SEED_ENTRIES: usize = 1_000;
const MAX_SEED_KEY_BYTES: usize = 128;

impl IdentitySeedPort for SqliteKernel {
    fn apply_identity_seed(
        &self,
        command: ApplyIdentitySeedCommand,
    ) -> ApplicationResult<ApplyIdentitySeedOutcome> {
        let capability = CapabilityKey::ApplyIdentitySeed;
        let correlation_id = command.correlation_id();
        if command.manifest().version().trim().is_empty()
            || command.manifest().entries().len() > MAX_SEED_ENTRIES
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
                || entry.key().len() > MAX_SEED_KEY_BYTES
                || !keys.insert(entry.key())
                || entry.identifiers().is_empty()
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
