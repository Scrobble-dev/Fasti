mod generate;
mod registry;

use anyhow::bail;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "cargo xtask")]
#[command(about = "Fasti repository-local verification tasks")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Contract generation and verification tasks
    Contract {
        #[command(subcommand)]
        command: ContractCommand,
    },
}

#[derive(Subcommand)]
enum ContractCommand {
    /// Validate authored capability ownership without claiming complete B1 conformance
    ValidateRegistry,
    /// Regenerate deterministic checked-in contract artifacts
    Generate,
    /// Verify every B1 contract surface and emit a receipt only after all gates pass
    Verify {
        /// Require Cargo and package lockfiles throughout nested checks
        #[arg(long)]
        locked: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let root = workspace_root()?;
    match cli.command {
        Command::Contract {
            command: ContractCommand::ValidateRegistry,
        } => {
            let summary = registry::validate(&root)?;
            println!(
                "PASS: contract_version={} capabilities={} surface_profiles={}",
                summary.contract_version, summary.capability_count, summary.surface_profile_count
            );
            Ok(())
        }
        Command::Contract {
            command: ContractCommand::Generate,
        } => {
            let artifacts = generate::generate_checked_in(&root)?;
            println!("PASS: generated {} contract artifacts", artifacts.len());
            Ok(())
        }
        Command::Contract {
            command: ContractCommand::Verify { locked },
        } => {
            let summary = registry::validate(&root)?;
            let first_directory = tempfile::tempdir()?;
            let second_directory = tempfile::tempdir()?;
            let first = generate::generate_to(&root, first_directory.path())?;
            let second = generate::generate_to(&root, second_directory.path())?;

            generate::compare_outputs(
                first_directory.path(),
                second_directory.path(),
                &first,
                &second,
            )?;
            generate::verify_checked_in(&root, &first)?;

            let lock_state = if locked {
                "locked"
            } else {
                "lock enforcement not requested"
            };
            bail!(
                "B1 contract verification is incomplete ({lock_state}; registry {} with {} capabilities and {} profiles is valid; checked-in OpenAPI, JSON Schemas, and public registry are deterministic): AsyncAPI, JSON-LD expansion, OKF, examples, UAT ownership, generated SDK parity, mutation sentinels, and black-box package gates are not all executable; no verification receipt was emitted",
                summary.contract_version,
                summary.capability_count,
                summary.surface_profile_count
            )
        }
    }
}

fn workspace_root() -> anyhow::Result<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("xtask must live directly under the workspace root"))
}
