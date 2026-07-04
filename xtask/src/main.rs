use clap::{Parser, Subcommand};
use xtask::SyncArgs;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Sync(SyncArgs),
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Sync(sync) => {
            if sync.run().await {
                std::process::exit(1);
            };
        }
    }
}
