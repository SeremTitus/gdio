mod config;
mod commands;
mod github;
mod godot;
mod project;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "gdio",
    about = "CLI tool for managing Godot Engine projects, editor versions and export templates.\n\nRun without arguments inside a Godot project directory to open it with the appropriate editor.",
    version,
    disable_version_flag = true
)]
struct Cli {
    /// Print version
    #[arg(short = 'v', long = "version", action = clap::ArgAction::SetTrue)]
    version: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
#[command(rename_all = "lowercase")]
enum Commands {
    /// List registered Godot editors
    List,

    /// Open current project in game mode
    Game,

    /// Show disk space used by gdio
    Cost,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.version {
        println!("gdio {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let mut config = config::Config::load()?;

    match cli.command {
        None => {
            commands::default::run(&mut config)?;
        }
        Some(cmd) => match cmd {
            Commands::List => {
                commands::list::run(&config)?;
            }
            Commands::Game => {
                commands::game::run(&mut config)?;
            }
            Commands::Cost => {
                commands::cost::run(&config)?;
            }
        },
    }

    Ok(())
}