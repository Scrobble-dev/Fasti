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
            command: ContractCommand::Verify { locked },
        } => {
            let _summary = registry::validate(&root)?;
            let lock_state = if locked {
                "locked"
            } else {
                "lock enforcement not requested"
            };
            bail!(
                "B1 contract verification is incomplete ({lock_state}): schemas, Utoipa OpenAPI, AsyncAPI, JSON-LD expansion, OKF, examples, UAT ownership, generated SDK parity, deterministic generation, and mutation sentinels are not all executable; no verification receipt was emitted"
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
