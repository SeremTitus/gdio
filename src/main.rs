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
    #[arg(short = 'v', long = "version", action = clap::ArgAction::SetTrue)]
    version: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
#[command(rename_all = "lowercase")]
enum Commands {
    Add {
        target: String,
        #[arg(short, long)]
        path: Option<String>,
        #[arg(long)]
        csharp: bool,
    },
    List,
    Remove {
        target: Option<String>,
    },
    Bind {
        target: Option<String>,
    },
    Game,
    Uninstall {
        #[arg(long)]
        keep: bool,
    },
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
            Commands::Add { target, path, csharp } => {
                commands::add::run(&target, path.as_deref(), csharp, &mut config)?;
            }
            Commands::List => {
                commands::list::run(&config)?;
            }
            Commands::Remove { target } => {
                commands::remove::run(target.as_deref(), &mut config)?;
            }
            Commands::Bind { target } => {
                commands::bind::run(target.as_deref(), &mut config)?;
            }
            Commands::Game => {
                commands::game::run(&mut config)?;
            }
            Commands::Uninstall { keep } => {
                commands::uninstall::run(keep)?;
            }
            Commands::Cost => {
                commands::cost::run(&config)?;
            }
        },
    }

    Ok(())
}