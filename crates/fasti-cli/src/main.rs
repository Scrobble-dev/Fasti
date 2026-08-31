mod capabilities;

use capabilities::{CapabilityCatalog, CliFailure, OutputFormat};
use clap::{Parser, Subcommand};
use fasti_application::CapabilityKey;
use fasti_contracts::public_capability_id;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "fasti")]
#[command(about = "Fasti records media activity; it does not play media", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Operate Fasti human-account access on this installation
    Access {
        #[command(subcommand)]
        command: AccessCommand,
    },
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
enum AccessCommand {
    /// Establish the first Fasti administrator through TrailBase on Unix
    BootstrapAdministrator {
        /// Private Fasti data root; the daemon must be stopped
        #[arg(long)]
        data_root: PathBuf,
        /// Private TrailBase root containing the verified installation receipt
        #[arg(long)]
        trailbase_root: PathBuf,
    },
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

fn unavailable(command: &str, capability: CapabilityKey) -> CliFailure {
    let body = capability.runtime_body().as_str();
    CliFailure::new(
        "capability_unavailable",
        public_capability_id(capability),
        format!(
            "{command} is not available in the current runtime; it is owned by {body}. No data was changed and no success receipt was emitted."
        ),
        format!("Wait for {body} public activation in this runtime before retrying {command}."),
    )
}

fn access_failure(code: fasti_application::ProblemCode) -> CliFailure {
    let next_action = match code {
        fasti_application::ProblemCode::TrailBaseSessionCleanupFailed => {
            "Do not retry blindly. Revoke the temporary TrailBase session in TrailBase administration, then inspect the Fasti ceremony evidence."
        }
        fasti_application::ProblemCode::TrailBaseTrustUnavailable => {
            "Repair or activate the pinned TrailBase installation, then retry."
        }
        fasti_application::ProblemCode::TrailBaseProofInvalid => {
            "Paste the complete, unchanged callback URL from the current TrailBase ceremony."
        }
        fasti_application::ProblemCode::Forbidden => {
            "Inspect whether the first administrator already exists before retrying."
        }
        fasti_application::ProblemCode::StorageUnavailable => {
            "Keep both private roots unchanged, correct the storage condition, and inspect the ceremony evidence before retrying."
        }
        _ => "Keep both private roots unchanged and inspect the recorded ceremony evidence before retrying.",
    };
    let safe_state = if code == fasti_application::ProblemCode::TrailBaseSessionCleanupFailed {
        "cleanup_uncertain_no_fasti_identity"
    } else {
        "no_new_active_fasti_session"
    };
    CliFailure::operation(
        code.as_str(),
        public_capability_id(CapabilityKey::AccessIdentityBootstrap),
        safe_state,
        format!("Administrator bootstrap stopped with {}.", code.as_str()),
        next_action,
    )
}

fn cancel_started(
    runtime: &fasti_api::LocalOperatorAccessRuntime,
    started: fasti_api::StartedFirstAdministratorBootstrap,
    original: CliFailure,
) -> CliFailure {
    match runtime.cancel_first_administrator_bootstrap(started) {
        Ok(()) => original,
        Err(code) => CliFailure::operation(
            code.as_str(),
            public_capability_id(CapabilityKey::AccessIdentityBootstrap),
            "ceremony_cleanup_unconfirmed",
            "Fasti could not confirm cancellation of the unfinished administrator ceremony.",
            "Do not retry blindly. Keep both private roots unchanged and inspect the ceremony evidence.",
        ),
    }
}

fn read_bounded_line(
    reader: &mut impl BufRead,
    maximum_bytes: usize,
) -> Result<String, CliFailure> {
    let mut value = String::with_capacity(maximum_bytes);
    loop {
        let buffer = reader.fill_buf().map_err(|_| {
            CliFailure::local(
                "input_failed",
                "Fasti could not read the TrailBase callback URL.",
                "Retry the command and paste the complete callback URL.",
            )
        })?;
        if buffer.is_empty() {
            break;
        }
        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(buffer.len(), |index| index + 1);
        let content = newline.map_or(buffer, |index| &buffer[..index]);
        if value.len() + content.len() > maximum_bytes {
            return Err(CliFailure::local(
                "input_invalid",
                "The callback URL exceeded the fixed C1 callback length.",
                "Paste only the complete callback URL from the current ceremony.",
            ));
        }
        value.push_str(std::str::from_utf8(content).map_err(|_| {
            CliFailure::local(
                "input_invalid",
                "The callback URL was not valid UTF-8.",
                "Paste the unchanged callback URL from the current ceremony.",
            )
        })?);
        reader.consume(consumed);
        if newline.is_some() {
            break;
        }
    }
    if value.ends_with('\r') {
        value.pop();
    }
    Ok(value)
}

#[cfg(unix)]
fn read_callback_url() -> Result<String, CliFailure> {
    use rustix::termios::{tcgetattr, tcsetattr, LocalModes, OptionalActions, Termios};
    use std::fs::{File, OpenOptions};
    use std::io::BufReader;

    struct EchoGuard<'a> {
        tty: &'a File,
        original: Termios,
    }

    impl Drop for EchoGuard<'_> {
        fn drop(&mut self) {
            let _ = tcsetattr(self.tty, OptionalActions::Now, &self.original);
        }
    }

    let tty = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .map_err(|_| {
            CliFailure::local(
                "terminal_required",
                "Administrator bootstrap requires an interactive terminal.",
                "Run this command from the installation user's terminal.",
            )
        })?;
    let original = tcgetattr(&tty).map_err(|_| {
        CliFailure::local(
            "terminal_unavailable",
            "Fasti could not read the terminal input settings.",
            "Check the terminal and retry the command.",
        )
    })?;
    let mut hidden = original.clone();
    hidden
        .local_modes
        .remove(LocalModes::ECHO | LocalModes::ECHONL);
    tcsetattr(&tty, OptionalActions::Now, &hidden).map_err(|_| {
        CliFailure::local(
            "terminal_unavailable",
            "Fasti could not protect the callback input from terminal echo.",
            "Check the terminal and retry the command.",
        )
    })?;
    let _guard = EchoGuard {
        tty: &tty,
        original,
    };
    let maximum_bytes = fasti_api::FASTI_ACCESS_CALLBACK_URL.len() + "?code=".len() + 48;
    let value = read_bounded_line(&mut BufReader::new(&tty), maximum_bytes)?;
    if value.is_empty() {
        return Err(CliFailure::local(
            "input_cancelled",
            "No callback URL was provided. The unfinished ceremony will be cancelled.",
            "Retry when the TrailBase account is ready.",
        ));
    }
    Ok(value)
}

#[cfg(not(unix))]
fn read_callback_url() -> Result<String, CliFailure> {
    Err(CliFailure::local(
        "platform_unavailable",
        "Protected callback input is not implemented on this platform.",
        "Use the supported Unix CLI host or wait for the packaged-host follow-up.",
    ))
}

async fn bootstrap_administrator(
    data_root: &Path,
    trailbase_root: &Path,
) -> Result<String, CliFailure> {
    let kernel = Arc::new(fasti_store::SqliteKernel::open(data_root).map_err(|error| {
        CliFailure::local(
            "data_root_unavailable",
            format!("Fasti could not lock and open the data root: {error}"),
            "Stop fastid for this data root, check its owner-only permissions, and retry.",
        )
    })?);
    let runtime =
        fasti_api::LocalOperatorAccessRuntime::new(kernel, trailbase_root).map_err(|error| {
            CliFailure::local(
                "trailbase_installation_unavailable",
                format!("Fasti could not verify the TrailBase installation: {error}"),
                "Repair or activate the pinned TrailBase installation, then retry.",
            )
        })?;
    let started = runtime
        .start_first_administrator_bootstrap()
        .map_err(access_failure)?;
    if writeln!(
        io::stdout().lock(),
        "Open this TrailBase authorization URL:\n{}\n\nThe ceremony expires at {}.",
        started.authorization_url(),
        started.expires_at().to_rfc3339(),
    )
    .is_err()
    {
        return Err(cancel_started(
            &runtime,
            started,
            CliFailure::local(
                "output_failed",
                "Fasti could not write the authorization URL.",
                "Check the terminal output and retry.",
            ),
        ));
    }
    if writeln!(
        io::stderr().lock(),
        "After TrailBase redirects, paste the complete callback URL here. Input is hidden:"
    )
    .is_err()
    {
        return Err(cancel_started(
            &runtime,
            started,
            CliFailure::local(
                "output_failed",
                "Fasti could not write the callback prompt.",
                "Check the terminal output and retry.",
            ),
        ));
    }
    loop {
        let callback_url = match read_callback_url() {
            Ok(value) => value,
            Err(error) => {
                return Err(cancel_started(&runtime, started, error));
            }
        };
        match runtime
            .complete_first_administrator_bootstrap(&started, &callback_url)
            .await
        {
            Ok(()) => break,
            Err(fasti_application::ProblemCode::TrailBaseProofInvalid)
                if chrono::Utc::now() < started.expires_at() =>
            {
                if writeln!(
                    io::stderr().lock(),
                    "That was not the exact callback URL. Paste the unchanged URL from this ceremony:"
                )
                .is_err()
                {
                    return Err(cancel_started(&runtime, started, CliFailure::local(
                        "output_failed",
                        "Fasti could not write the callback retry prompt.",
                        "Cancel this command and retry from a working terminal.",
                    )));
                }
            }
            Err(fasti_application::ProblemCode::TrailBaseProofInvalid) => {
                let error = access_failure(fasti_application::ProblemCode::TrailBaseProofInvalid);
                return Err(cancel_started(&runtime, started, error));
            }
            Err(code) => return Err(access_failure(code)),
        }
    }
    Ok("The first Fasti administrator is established. No active browser session was returned. Start the normal host and use Sign in to create one.".to_owned())
}

async fn execute(cli: Cli) -> Result<String, CliFailure> {
    match cli.command {
        Commands::Access { command } => match command {
            AccessCommand::BootstrapAdministrator {
                data_root,
                trailbase_root,
            } => bootstrap_administrator(&data_root, &trailbase_root).await,
        },
        Commands::Capability { command } => {
            let catalog = CapabilityCatalog::load()?;
            match command {
                CapabilityCommand::List { output } => catalog.list(output),
                CapabilityCommand::Show { id, output } => catalog.show(&id, output),
            }
        }
        Commands::Export { output: _ } => {
            Err(unavailable("export", CapabilityKey::ExportWorkspace))
        }
        Commands::Restore { input: _ } => {
            Err(unavailable("restore", CapabilityKey::RestoreWorkspace))
        }
        Commands::Verify => Err(unavailable("verify", CapabilityKey::VerifyWorkspace)),
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    match execute(Cli::parse()).await {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_administrator_cli_accepts_only_private_root_locations() {
        let parsed = Cli::try_parse_from([
            "fasti",
            "access",
            "bootstrap-administrator",
            "--data-root",
            "/srv/fasti",
            "--trailbase-root",
            "/srv/trailbase",
        ])
        .expect("bootstrap command");
        assert!(matches!(
            parsed.command,
            Commands::Access {
                command: AccessCommand::BootstrapAdministrator { .. }
            }
        ));
        assert!(Cli::try_parse_from([
            "fasti",
            "access",
            "bootstrap-administrator",
            "--data-root",
            "/srv/fasti",
            "--trailbase-root",
            "/srv/trailbase",
            "--password",
            "not-allowed",
        ])
        .is_err());
    }

    #[test]
    fn callback_input_is_bounded_and_preserves_non_line_whitespace() {
        let exact = format!(
            "{}?code={}\n",
            fasti_api::FASTI_ACCESS_CALLBACK_URL,
            "aB3".repeat(16)
        );
        assert_eq!(
            read_bounded_line(&mut exact.as_bytes(), exact.len()).expect("bounded callback"),
            exact.strip_suffix('\n').expect("line ending")
        );
        let spaced = format!(" {}", exact);
        assert!(
            read_bounded_line(&mut spaced.as_bytes(), exact.len() - 1).is_err(),
            "leading whitespace must not be silently trimmed"
        );
    }
}
