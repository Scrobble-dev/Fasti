mod evidence;
mod generate;
mod integration;
mod orchestration;
mod registry;
mod verify;

use clap::{Parser, Subcommand, ValueEnum};
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

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
    /// Focused, offline integration contract checks
    Integration {
        #[command(subcommand)]
        command: IntegrationCommand,
    },
}

#[derive(Subcommand)]
enum IntegrationCommand {
    /// Validate one authored provider manifest and its four deterministic fixtures
    Check {
        path: PathBuf,
        #[arg(long, value_enum, default_value = "human")]
        output: integration::OutputFormat,
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
    B8a,
    B8b,
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

/// Parses command-line arguments, dispatches the selected workspace operation, and reports its result.
///
/// # Examples
///
/// ```
/// # fn main() -> std::io::Result<()> {
/// let status = std::process::Command::new("cargo")
///     .args(["xtask", "--help"])
///     .status()?;
/// assert!(status.success());
/// # Ok(())
/// # }
/// ```
fn main() -> ExitCode {
    let cli = Cli::parse();
    let root = match workspace_root() {
        Ok(root) => root,
        Err(error) => {
            eprintln!("Error: {error:#}");
            return ExitCode::FAILURE;
        }
    };
    match cli.command {
        Command::Integration {
            command: IntegrationCommand::Check { path, output },
        } => match integration::check(&root, &path) {
            Ok(report) => match integration::render_success(&report, output) {
                Ok(rendered) => match write_result(&rendered, false) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(error) => {
                        eprintln!("Error: failed to write integration check output: {error}");
                        ExitCode::FAILURE
                    }
                },
                Err(error) => {
                    eprintln!("Error: {error:#}");
                    ExitCode::FAILURE
                }
            },
            Err(failure @ integration::CheckFailure::Validation(_)) => {
                let exit_code = ExitCode::from(failure.exit_code());
                let integration::CheckFailure::Validation(problem) = failure else {
                    unreachable!("matched validation failure")
                };
                match integration::render_validation_failure(&problem, output) {
                    Ok(rendered) => match write_result(&rendered, true) {
                        Ok(()) => exit_code,
                        Err(error) => {
                            eprintln!("Error: failed to write integration check output: {error}");
                            ExitCode::FAILURE
                        }
                    },
                    Err(error) => {
                        eprintln!("Error: {error:#}");
                        ExitCode::FAILURE
                    }
                }
            }
            Err(integration::CheckFailure::Tool(error)) => {
                eprintln!("Error: integration check could not run: {error:#}");
                ExitCode::FAILURE
            }
        },
        command => match run_existing(command, &root) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("Error: {error:#}");
                ExitCode::FAILURE
            }
        },
    }
}

fn write_result(rendered: &str, stderr: bool) -> std::io::Result<()> {
    if stderr {
        std::io::stderr().write_all(rendered.as_bytes())
    } else {
        std::io::stdout().write_all(rendered.as_bytes())
    }
}

fn run_existing(command: Command, root: &std::path::Path) -> anyhow::Result<()> {
    match command {
        Command::Contract {
            command: ContractCommand::ValidateRegistry,
        } => {
            let summary = registry::validate(root)?;
            println!(
                "PASS: contract_version={} capabilities={} surface_profiles={}",
                summary.contract_version, summary.capability_count, summary.surface_profile_count
            );
            Ok(())
        }
        Command::Contract {
            command: ContractCommand::Generate,
        } => {
            let artifacts = generate::generate_checked_in(root)?;
            println!("PASS: generated {} contract artifacts", artifacts.len());
            Ok(())
        }
        Command::Contract {
            command: ContractCommand::Verify { locked },
        } => verify_contracts(root, locked),
        Command::Test {
            command: TestCommand::Pr,
        } => run_pr(root),
        Command::Test {
            command: TestCommand::Deep,
        } => run_deep(root),
        Command::Test {
            command: TestCommand::Milestone { body, manifest },
        } => run_milestone(root, body, manifest),
        Command::Evidence {
            command: EvidenceCommand::Schema,
        } => evidence::print_schema(),
        Command::Evidence {
            command: EvidenceCommand::Verify { manifest },
        } => evidence::verify(root, &manifest).map(|_| ()),
        Command::Integration { .. } => unreachable!("integration checks return before dispatch"),
    }
}

/// Runs the canonical B1 pull-request gate, including contract verification and portable tests.
///
/// # Arguments
///
/// * `root` - Path to the workspace root.
///
/// # Errors
///
/// Returns an error if contract verification or portable B1 tests fail.
///
/// # Examples
///
/// ```no_run
/// # fn main() -> anyhow::Result<()> {
/// run_pr(std::path::Path::new("."))?;
/// # Ok(())
/// # }
/// ```
fn run_pr(root: &std::path::Path) -> anyhow::Result<()> {
    verify_contracts(root, true)?;
    orchestration::run_portable_b1(root)?;
    println!("PASS: canonical B1 pull-request gate");
    Ok(())
}

/// Runs the pull-request gate and all deep checks applicable to the current B1 body.
///
/// # Errors
///
/// Returns an error if the pull-request gate or any deep check fails.
///
/// # Examples
///
/// ```no_run
/// run_deep(std::path::Path::new("."))?;
/// # Ok::<(), anyhow::Error>(())
/// ```
fn run_deep(root: &std::path::Path) -> anyhow::Result<()> {
    run_pr(root)?;
    orchestration::run_deep_b1(root)?;
    println!("PASS: every deep check applicable to the current B1 body");
    Ok(())
}

/// Runs the selected milestone gate and creates its evidence manifest when applicable.
///
/// # Arguments
///
/// * `manifest` - Optional output path for the evidence manifest. Defaults to the
///   milestone-specific path under `target/fasti-evidence`.
///
/// # Examples
///
/// ```
/// let result = run_milestone(
///     std::path::Path::new("."),
///     BodyArg::B8a,
///     None,
/// );
/// assert!(result.is_err());
/// ```
///
/// # Errors
///
/// Returns an error if the milestone is unavailable, a prerequisite is missing
/// or invalid, or gate verification fails. B8b additionally requires a passing
/// B8a manifest.
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
        BodyArg::B8a => anyhow::bail!(
            "B8a milestone evidence formalization is not implemented; B8a is a prerequisite for B8b and is out of scope for this gate"
        ),
        BodyArg::B8b => {
            run_deep(root)?;
            evidence::verify_b8a_prerequisite(root)?;
            let manifest =
                manifest.unwrap_or_else(|| root.join("target/fasti-evidence/b8b-manifest.json"));
            evidence::create_b8b_milestone_manifest(root, &manifest).map(|_| ())
        }
    }
}

/// Validates B1 software contracts and writes a verification receipt when all checks succeed.
///
/// This verifies the capability registry, deterministic generated artifacts, checked-in artifacts,
/// registry examples, and the remaining contract checks. Any existing receipt is removed when
/// verification fails.
///
/// # Arguments
///
/// * `root` - Path to the workspace root.
/// * `locked` - Whether dependency verification must use locked dependency graphs.
///
/// # Examples
///
/// ```no_run
/// # fn main() -> anyhow::Result<()> {
/// let root = workspace_root()?;
/// verify_contracts(&root, true)?;
/// # Ok(())
/// # }
/// ```
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

/// Locates the workspace root directory containing the `xtask` crate.
///
/// # Errors
///
/// Returns an error when the `xtask` crate manifest directory has no parent.
///
/// # Examples
///
/// ```
/// let root = workspace_root()?;
/// assert!(root.is_dir());
/// # Ok::<(), anyhow::Error>(())
/// ```
fn workspace_root() -> anyhow::Result<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("xtask must live directly under the workspace root"))
}
