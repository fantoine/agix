use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};

#[derive(ValueEnum, Clone, Debug)]
enum Scope {
    Global,
    Local,
}

impl Scope {
    fn as_str(&self) -> &'static str {
        match self {
            Scope::Global => "global",
            Scope::Local => "local",
        }
    }
}

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
        #[arg(long, default_value = "local")]
        scope: Scope,
        /// Pre-select CLIs (skips the interactive menu). Repeatable.
        #[arg(long, num_args = 1..)]
        cli: Vec<String>,
        /// Skip the interactive menu entirely.
        #[arg(long)]
        no_interactive: bool,
    },
    Install {
        #[arg(long, default_value = "local")]
        scope: Scope,
    },
    Add {
        /// Source type: local | github | git | marketplace
        source_type: String,
        /// Source value (path, org/repo, URL, <org/repo>@<plugin>, ...)
        source_value: String,
        #[arg(long, default_value = "local")]
        scope: Scope,
        #[arg(long, num_args = 1..)]
        cli: Vec<String>,
        #[arg(long)]
        version: Option<String>,
    },
    Remove {
        name: String,
        #[arg(long, default_value = "local")]
        scope: Scope,
        #[arg(long, num_args = 1..)]
        cli: Vec<String>,
    },
    Update {
        name: Option<String>,
        #[arg(long, default_value = "local")]
        scope: Scope,
    },
    List {
        #[arg(long, default_value = "local")]
        scope: Scope,
    },
    Outdated {
        #[arg(long, default_value = "local")]
        scope: Scope,
    },
    Check,
    Doctor,
    Export {
        #[arg(long, default_value = "local")]
        scope: Scope,
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
            scope,
            cli,
            no_interactive,
        } => agix::commands::init::run(scope.as_str(), cli, no_interactive).await,
        Commands::Install { scope } => agix::commands::install::run(scope.as_str()).await,
        Commands::Add {
            source_type,
            source_value,
            scope,
            cli,
            version,
        } => {
            agix::commands::add::run(source_type, source_value, scope.as_str(), cli, version).await
        }
        Commands::Remove { name, scope, cli } => {
            agix::commands::remove::run(name, scope.as_str(), cli).await
        }
        Commands::Update { name, scope } => agix::commands::update::run(name, scope.as_str()).await,
        Commands::List { scope } => agix::commands::list::run(scope.as_str()).await,
        Commands::Outdated { scope } => agix::commands::outdated::run(scope.as_str()).await,
        Commands::Check => agix::commands::check::run().await,
        Commands::Doctor => agix::commands::doctor::run().await,
        Commands::Export { scope, all, output } => {
            agix::commands::export::run(scope.as_str(), all, output).await
        }
    }
}
