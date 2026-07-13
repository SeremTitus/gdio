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

    #[arg(short = 'r', action = clap::ArgAction::SetTrue)]
    recent: bool,

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
    New {
        name: String,
    },
    Projects,
    Build {
        #[arg(long)]
        windows: bool,
        #[arg(long)]
        linux: bool,
        #[arg(long)]
        web: bool,
        #[arg(long)]
        macos: bool,
        #[arg(long)]
        ios: bool,
        #[arg(long)]
        android: bool,
        #[arg(long)]
        debug: bool,
    },
    Uninstall {
        #[arg(long)]
        keep: bool,
    },
    Cost,
}

fn main() -> anyhow::Result<()> {
    let raw_args: Vec<String> = std::env::args().collect();
    let args: Vec<String> = raw_args.iter().map(|a| a.to_lowercase()).collect();
    let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let cli = Cli::parse_from(args);

    if cli.version {
        println!("gdio {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let mut config = config::Config::load()?;

    if cli.recent {
        commands::recent::run(&mut config)?;
        return Ok(());
    }

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
            Commands::New { name } => {
                commands::new::run(&name, &mut config)?;
            }
            Commands::Projects => {
                commands::projects::run(&mut config)?;
            }
            Commands::Build { windows, linux, web, macos, ios, android, debug } => {
                commands::build::run(windows, linux, web, macos, ios, android, debug, &mut config)?;
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