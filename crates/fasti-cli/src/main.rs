mod capabilities;

use capabilities::{CapabilityCatalog, CliFailure, OutputFormat};
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "fasti")]
#[command(about = "Fasti records media activity; it does not play media", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Inspect checked-in public resources through system.capabilities.discover
    Capability {
        #[command(subcommand)]
        command: CapabilityCommand,
    },
    /// Reserved for the B3 verified workspace export capability
    Export {
        #[arg(short, long)]
        output: String,
    },
    /// Reserved for the B3 clean-restore capability
    Restore {
        #[arg(short, long)]
        input: String,
    },
    /// Reserved for the B3 workspace verification capability
    Verify,
}

#[derive(Subcommand)]
enum CapabilityCommand {
    /// List public capability resources (system.capabilities.discover)
    List {
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        output: OutputFormat,
    },
    /// Show one public capability resource (system.capabilities.discover)
    Show {
        /// Stable public capability identifier
        id: String,
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        output: OutputFormat,
    },
}

fn unavailable(command: &str, body: &str) -> CliFailure {
    CliFailure::new(
        "capability_unavailable",
        capability_for_stub(command),
        format!(
            "{command} is not available in B0; it is owned by {body}. No data was changed and no success receipt was emitted."
        ),
        format!("Wait for the {body} implementation gate before retrying {command}."),
    )
}

fn capability_for_stub(command: &str) -> &'static str {
    match command {
        "export" => "portability.workspace.export",
        "restore" => "portability.workspace.restore",
        "verify" => "portability.workspace.verify",
        _ => "system.capabilities.discover",
    }
}

fn execute(cli: Cli) -> Result<String, CliFailure> {
    match cli.command {
        Commands::Capability { command } => {
            let catalog = CapabilityCatalog::load()?;
            match command {
                CapabilityCommand::List { output } => catalog.list(output),
                CapabilityCommand::Show { id, output } => catalog.show(&id, output),
            }
        }
        Commands::Export { output: _ } => Err(unavailable("export", "B3")),
        Commands::Restore { input: _ } => Err(unavailable("restore", "B3")),
        Commands::Verify => Err(unavailable("verify", "B3")),
    }
}

fn main() -> ExitCode {
    match execute(Cli::parse()) {
        Ok(output) => {
            if writeln!(io::stdout().lock(), "{output}").is_ok() {
                ExitCode::SUCCESS
            } else {
                let error = CliFailure::local(
                    "output_failed",
                    "The capability result could not be written to stdout.",
                    "Check the output destination and retry the command.",
                );
                let _ = writeln!(io::stderr().lock(), "{error}");
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "{error}");
            ExitCode::FAILURE
        }
    }
}
