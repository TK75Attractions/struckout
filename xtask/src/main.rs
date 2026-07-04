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

const DB_PATH_DEFAULT: &str = "sqlite:///home/taichi765/.config/struckout/xtask.db";

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Sync(args) => match args.run().await {
            Ok(_) => {}
            Err(e) => {
                eprintln!("sync failed: {:?}", e);
                std::process::exit(1);
            }
        },
    }
}
