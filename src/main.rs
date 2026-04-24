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
        #[arg(short = 'g', long)]
        global: bool,
        /// Pre-select CLIs (skips the interactive menu). Repeatable.
        #[arg(long, num_args = 1..)]
        cli: Vec<String>,
        #[arg(long)]
        no_interactive: bool,
    },
    Install {
        #[arg(short = 'g', long)]
        global: bool,
    },
    Add {
        /// Source type: local | github | git | marketplace
        source_type: String,
        /// Source value (path, org/repo, URL, <org/repo>@<plugin>, ...)
        source_value: String,
        #[arg(short = 'g', long)]
        global: bool,
        #[arg(long, num_args = 1..)]
        cli: Vec<String>,
        #[arg(long)]
        version: Option<String>,
    },
    Remove {
        name: String,
        #[arg(short = 'g', long)]
        global: bool,
        #[arg(long, num_args = 1..)]
        cli: Vec<String>,
    },
    Update {
        name: Option<String>,
        #[arg(short = 'g', long)]
        global: bool,
    },
    List {
        #[arg(short = 'g', long)]
        global: bool,
    },
    Outdated {
        #[arg(short = 'g', long)]
        global: bool,
    },
    Check,
    Doctor {
        #[arg(short = 'g', long)]
        global: bool,
    },
    Export {
        #[arg(short = 'g', long)]
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
        Commands::Init {
            global,
            cli,
            no_interactive,
        } => agix::commands::init::run(global, cli, no_interactive).await,
        Commands::Install { global } => agix::commands::install::run(global).await,
        Commands::Add {
            source_type,
            source_value,
            global,
            cli,
            version,
        } => agix::commands::add::run(source_type, source_value, global, cli, version).await,
        Commands::Remove { name, global, cli } => {
            agix::commands::remove::run(name, global, cli).await
        }
        Commands::Update { name, global } => agix::commands::update::run(name, global).await,
        Commands::List { global } => agix::commands::list::run(global).await,
        Commands::Outdated { global } => agix::commands::outdated::run(global).await,
        Commands::Check => agix::commands::check::run().await,
        Commands::Doctor { global } => agix::commands::doctor::run(global).await,
        Commands::Export {
            global,
            all,
            output,
        } => agix::commands::export::run(global, all, output).await,
    }
}
