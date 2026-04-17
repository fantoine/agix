use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "agix",
    about = "Agent Graph IndeX \u{2014} package manager for AI CLI tools",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init {
        #[arg(long)]
        global: bool,
    },
    Install {
        #[arg(long)]
        global: bool,
    },
    Add {
        source: String,
        #[arg(long)]
        global: bool,
        #[arg(long)]
        cli: Option<String>,
        #[arg(long)]
        version: Option<String>,
    },
    Remove {
        name: String,
        #[arg(long)]
        global: bool,
    },
    Update {
        name: Option<String>,
        #[arg(long)]
        global: bool,
    },
    List {
        #[arg(long)]
        global: bool,
    },
    Outdated {
        #[arg(long)]
        global: bool,
    },
    Check,
    Doctor,
    Export {
        #[arg(long)]
        global: bool,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        output: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init { global } => agix::commands::init::run(global).await,
        Commands::Install { global } => agix::commands::install::run(global).await,
        Commands::Add {
            source,
            global,
            cli,
            version,
        } => agix::commands::add::run(source, global, cli, version).await,
        Commands::Remove { name, global } => agix::commands::remove::run(name, global).await,
        Commands::Update { name, global } => agix::commands::update::run(name, global).await,
        Commands::List { global } => agix::commands::list::run(global).await,
        Commands::Outdated { global } => agix::commands::outdated::run(global).await,
        Commands::Check => agix::commands::check::run().await,
        Commands::Doctor => agix::commands::doctor::run().await,
        Commands::Export {
            global,
            all,
            output,
        } => agix::commands::export::run(global, all, output).await,
    }
}
