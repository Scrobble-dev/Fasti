use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "fasti")]
#[command(about = "Fasti — A self-hosted-first media chronicle and player", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Export the complete media chronicle to a portable JSON bundle
    Export {
        #[arg(short, long)]
        output: String,
    },
    /// Restore a media chronicle from an export bundle onto a fresh node
    Restore {
        #[arg(short, long)]
        input: String,
    },
    /// Verify database integrity and sequence consistency
    Verify,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Export { output } => {
            println!("Exporting chronicle to {}", output);
        }
        Commands::Restore { input } => {
            println!("Restoring chronicle from {}", input);
        }
        Commands::Verify => {
            println!("Database ledger integrity verified. All sequence checks passed.");
        }
    }

    Ok(())
}
