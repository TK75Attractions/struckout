use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[cfg(feature = "sqlite")]
    Sync(xtask::SyncArgs),
    #[command(about = "Create new screen in touchpanel")]
    New(xtask::NewArgs),
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match cli.command {
        #[cfg(feature = "sqlite")]
        Commands::Sync(sync) => {
            if sync.run().await {
                std::process::exit(1);
            };
        }
        Commands::New(new) => {
            if let Err(e) = new.run() {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    }
}
