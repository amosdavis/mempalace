use clap::{Parser, Subcommand};
use anyhow::Result;

mod commands;

#[derive(Parser)]
#[command(name = "mempalace", about = "MemPalace — AI memory system", version = "3.3.3")]
struct Cli {
    #[arg(long, global = true)]
    palace: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize or verify the palace
    Init,
    /// Mine a project directory into the palace
    Mine {
        /// Project directory to mine
        #[arg(default_value = ".")]
        dir: String,
        /// Wing to store in
        #[arg(long, default_value = "wing_code")]
        wing: String,
        /// Force re-mining
        #[arg(long)]
        force: bool,
    },
    /// Search the palace
    Search {
        query: String,
        #[arg(long)]
        wing: Option<String>,
        #[arg(long)]
        room: Option<String>,
        #[arg(long, short, default_value = "5")]
        n: usize,
    },
    /// Show status and progress of an in-progress or most recent mine operation
    Progress,
    /// Show palace status
    Status,
    /// Start MCP server over stdio (or pass --install to print setup command)
    Mcp {
        /// Print Claude MCP install command instead of starting server
        #[arg(long)]
        install: bool,
    },
    /// Wake-up summary
    WakeUp,
    /// Hook handlers for Claude Code / Codex CLI integration
    Hook {
        #[command(subcommand)]
        subcommand: HookCommand,
    },
}

#[derive(Subcommand)]
enum HookCommand {
    /// Handle Claude Code Stop hook (count messages, optionally block for save)
    Stop,
    /// Handle Claude Code PreCompact hook (mine transcript before compaction)
    Precompact,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let palace = cli.palace.as_deref();

    match cli.command {
        Commands::Init => commands::init::run(palace),
        Commands::Mine { dir, wing, force } => commands::mine::run(&dir, &wing, force, palace),
        Commands::Search { query, wing, room, n } => {
            commands::search::run(&query, wing.as_deref(), room.as_deref(), n, palace)
        }
        Commands::Progress => commands::progress::run(),
        Commands::Status => commands::status::run(palace),
        Commands::Mcp { install } => {
            if install {
                commands::mcp::print_install_cmd();
                Ok(())
            } else {
                commands::mcp::run_server(palace)
            }
        }
        Commands::WakeUp => commands::wake_up::run(palace),
        Commands::Hook { subcommand } => match subcommand {
            HookCommand::Stop => commands::hook::run_stop(),
            HookCommand::Precompact => commands::hook::run_precompact(),
        },
    }
}
