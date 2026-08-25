mod evidence;
mod generate;
mod orchestration;
mod registry;
mod verify;

use clap::{Parser, Subcommand, ValueEnum};
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
    /// Canonical contributor and milestone test orchestration
    Test {
        #[command(subcommand)]
        command: TestCommand,
    },
    /// Canonical digest-bound evidence operations
    Evidence {
        #[command(subcommand)]
        command: EvidenceCommand,
    },
}

#[derive(Subcommand)]
enum TestCommand {
    /// Run the complete, bounded pull-request gate
    Pr,
    /// Run the pull-request gate plus applicable deep checks
    Deep,
    /// Run a fail-closed milestone gate for one implementation body
    Milestone {
        #[arg(long, value_enum, ignore_case = true)]
        body: BodyArg,
        /// Override the canonical digest-bound evidence manifest path
        #[arg(long)]
        manifest: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum BodyArg {
    B0,
    B1,
    B2,
    B3,
}

#[derive(Subcommand)]
enum EvidenceCommand {
    /// Print the strict evidence-manifest schema and its canonical digest
    Schema,
    /// Verify schema, RFC 8785 manifest digest, file digests, source, and claims
    Verify { manifest: PathBuf },
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
        } => verify_contracts(&root, locked),
        Command::Test {
            command: TestCommand::Pr,
        } => run_pr(&root),
        Command::Test {
            command: TestCommand::Deep,
        } => run_deep(&root),
        Command::Test {
            command: TestCommand::Milestone { body, manifest },
        } => run_milestone(&root, body, manifest),
        Command::Evidence {
            command: EvidenceCommand::Schema,
        } => evidence::print_schema(),
        Command::Evidence {
            command: EvidenceCommand::Verify { manifest },
        } => evidence::verify(&root, &manifest).map(|_| ()),
    }
}

fn run_pr(root: &std::path::Path) -> anyhow::Result<()> {
    verify_contracts(root, true)?;
    orchestration::run_portable_b1(root)?;
    println!("PASS: canonical B1 pull-request gate");
    Ok(())
}

fn run_deep(root: &std::path::Path) -> anyhow::Result<()> {
    run_pr(root)?;
    orchestration::run_deep_b1(root)?;
    println!("PASS: every deep check applicable to the current B1 body");
    Ok(())
}

fn run_milestone(
    root: &std::path::Path,
    body: BodyArg,
    manifest: Option<PathBuf>,
) -> anyhow::Result<()> {
    match body {
        BodyArg::B1 => {
            run_deep(root)?;
            let manifest = manifest
                .unwrap_or_else(|| root.join("target/fasti-evidence/b1-manifest.json"));
            evidence::create_b1_milestone_manifest(root, &manifest).map(|_| ())
        }
        BodyArg::B0 => anyhow::bail!(
            "B0 predates the canonical milestone evidence manifest; create and verify a B0 manifest before claiming this gate"
        ),
        BodyArg::B2 | BodyArg::B3 => anyhow::bail!(
            "{} is not authorized while the fail-closed B1 milestone remains open",
            match body {
                BodyArg::B2 => "B2",
                BodyArg::B3 => "B3",
                _ => unreachable!(),
            }
        ),
    }
}

fn verify_contracts(root: &std::path::Path, locked: bool) -> anyhow::Result<()> {
    verify::clear_receipt(root)?;

    let result: anyhow::Result<()> = (|| {
        println!("RUN [registry.validate]: validate the authored capability registry");
        let summary = registry::validate(root)?;
        println!(
            "PASS [registry.validate]: contract_version={} capabilities={} surface_profiles={}",
            summary.contract_version, summary.capability_count, summary.surface_profile_count
        );

        println!("RUN [generation.first]: generate contracts in an isolated directory");
        let first_directory = tempfile::tempdir()?;
        let first = generate::generate_to(root, first_directory.path())?;
        println!(
            "PASS [generation.first]: generated {} artifacts",
            first.len()
        );

        println!("RUN [generation.second]: repeat isolated contract generation");
        let second_directory = tempfile::tempdir()?;
        let second = generate::generate_to(root, second_directory.path())?;
        println!(
            "PASS [generation.second]: generated {} artifacts",
            second.len()
        );

        println!("RUN [generation.deterministic]: compare isolated generated bytes");
        generate::compare_outputs(
            first_directory.path(),
            second_directory.path(),
            &first,
            &second,
        )?;
        println!("PASS [generation.deterministic]: isolated outputs are byte-identical");

        println!("RUN [generation.checked_in]: compare generated bytes and inventory to git");
        generate::verify_checked_in(root, &first)?;
        println!("PASS [generation.checked_in]: checked-in artifacts have no drift");

        println!("RUN [examples.inventory]: verify every registry example has one parseable file");
        let example_count = verify::verify_examples(root)?;
        println!("PASS [examples.inventory]: {example_count} governed examples are present");

        let facts = verify::VerificationFacts {
            contract_version: summary.contract_version,
            capability_count: summary.capability_count,
            surface_profile_count: summary.surface_profile_count,
            generated_artifact_count: first.len(),
            example_count,
        };
        let receipt = verify::run_and_write_receipt(root, locked, &facts)?;
        println!(
            "PASS: B1 software contract verification complete; receipt={}",
            receipt.display()
        );
        Ok(())
    })();

    if let Err(error) = result {
        if let Err(cleanup_error) = verify::clear_receipt(root) {
            return Err(anyhow::anyhow!(
                "{error:#}; additionally failed to remove the invalid verification receipt: {cleanup_error:#}"
            ));
        }
        return Err(
            error.context("B1 contract verification failed; no verification receipt was emitted")
        );
    }

    Ok(())
}

fn workspace_root() -> anyhow::Result<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("xtask must live directly under the workspace root"))
}
