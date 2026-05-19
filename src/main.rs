mod config;
mod hooks;
mod ops;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "free-my-agent",
    about = "Hide agent instruction files during work, restore them on commit"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Install git hooks into .git/hooks/
    Init,
    /// Add a glob pattern to managed list (default: local to this repo)
    Add {
        pattern: String,
        /// Add to global config (~/.config/free-my-agent/managed)
        #[arg(long)]
        global: bool,
    },
    /// Remove a glob pattern from managed list (default: local to this repo)
    Remove {
        pattern: String,
        /// Remove from global config
        #[arg(long)]
        global: bool,
    },
    /// List managed patterns
    List,
    /// Backup and delete managed files (free the agent)
    Free,
    /// Restore managed files from backup (called by pre-commit hook)
    Restore,
    /// Show which files are currently hidden
    Status,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Cmd::Init => hooks::install(),
        Cmd::Add { pattern, global } => config::add_pattern(&pattern, global),
        Cmd::Remove { pattern, global } => config::remove_pattern(&pattern, global),
        Cmd::List => {
            let global = config::read_global_patterns()?;
            let local = config::read_local_patterns()?;
            let mut any = false;
            if !global.is_empty() {
                println!("[global]");
                for p in &global {
                    println!("  {p}");
                }
                any = true;
            }
            if !local.is_empty() {
                println!("[local]");
                for p in &local {
                    println!("  {p}");
                }
                any = true;
            }
            if !any {
                println!("no patterns registered");
            }
            Ok(())
        }
        Cmd::Free => ops::free(),
        Cmd::Restore => ops::restore(),
        Cmd::Status => {
            let backup = config::backup_dir()?;
            let entries = std::fs::read_dir(&backup)?;
            let mut found = false;
            for entry in entries {
                let entry = entry?;
                let path = entry.path();
                let label = if path.is_dir() {
                    "hidden (dir)"
                } else {
                    "hidden"
                };
                println!("{label}: {}", entry.file_name().to_string_lossy());
                found = true;
            }
            if !found {
                println!("no files currently hidden");
            }
            Ok(())
        }
    }
}
