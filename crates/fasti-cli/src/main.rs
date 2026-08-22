use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "fasti")]
#[command(about = "Fasti records media activity; it does not play media", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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

fn unavailable(command: &str, body: &str) -> anyhow::Result<()> {
    anyhow::bail!(
        "{command} is not available in B0; it is owned by {body}. No data was changed and no success receipt was emitted."
    )
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Export { output: _ } => unavailable("export", "B3"),
        Commands::Restore { input: _ } => unavailable("restore", "B3"),
        Commands::Verify => unavailable("verify", "B3"),
    }
}
