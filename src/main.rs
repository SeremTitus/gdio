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

    /// Open the last project opened by gdio
    #[arg(short = 'r', action = clap::ArgAction::SetTrue)]
    recent: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
#[command(rename_all = "lowercase")]
enum Commands {
    /// Add a Godot editor (download by version or register local path)
    Add {
        /// Version to download (e.g., 4.7, 4.7-stable) or path to local executable
        target: String,

        /// Path to local Godot executable (when registering existing)
        #[arg(short, long)]
        path: Option<String>,

        /// Download the C# (mono) variant
        #[arg(long)]
        csharp: bool,
    },

    /// List registered Godot editors
    List,

    /// Remove a Godot editor
    Remove {
        /// Version or name of editor to remove (interactive if omitted)
        target: Option<String>,
    },

    /// Bind an editor to the current project
    Bind {
        /// Version or name of editor to bind (adds if not found, interactive if omitted)
        target: Option<String>,
    },

    /// Open current project in game mode
    Game,

    /// Open the last project opened by gdio
    Recent,

    /// List and open projects known to gdio
    Projects,

    /// Create a new Godot project
    New {
        /// Project name (creates subdirectory in current dir)
        name: String,
    },

    /// Export the current project
    Build {
        /// Export for Windows
        #[arg(long)]
        windows: bool,

        /// Export for Linux
        #[arg(long)]
        linux: bool,

        /// Export for Web
        #[arg(long)]
        web: bool,

        /// Export for macOS
        #[arg(long)]
        macos: bool,

        /// Export for iOS
        #[arg(long)]
        ios: bool,

        /// Export for Android
        #[arg(long)]
        android: bool,

        /// Export in debug mode
        #[arg(long)]
        debug: bool,
    },

    /// Upload project to itch.io via butler
    Up {
        /// Run interactive setup for itch.io upload configuration
        #[arg(long)]
        setup: bool,

        /// Upload for Windows
        #[arg(long)]
        windows: bool,

        /// Upload for Linux
        #[arg(long)]
        linux: bool,

        /// Upload for Web
        #[arg(long)]
        web: bool,

        /// Upload for macOS
        #[arg(long)]
        macos: bool,

        /// Upload for iOS
        #[arg(long)]
        ios: bool,

        /// Upload for Android
        #[arg(long)]
        android: bool,

        /// Export in debug mode before upload
        #[arg(long)]
        debug: bool,

        /// Interactively name zip files before upload
        #[arg(long)]
        name: bool,
    },

    /// Uninstall gdio and remove all its files
    Uninstall {
        /// Keep config and editor binaries
        #[arg(long)]
        keep: bool,
    },

    /// Show disk space used by gdio
    Cost,

    /// Manage export templates
    Templates {
        #[command(subcommand)]
        action: Option<TemplatesAction>,
    },
}

#[derive(Subcommand)]
#[command(rename_all = "lowercase")]
enum TemplatesAction {
    /// List installed export templates
    List,

    /// Download export templates for a version
    Add {
        /// Godot version (e.g., 4.7)
        #[arg(value_name = "GODOT_VERSION")]
        godot_version: String,

        /// Export for Windows
        #[arg(long)]
        windows: bool,

        /// Export for Linux
        #[arg(long)]
        linux: bool,

        /// Export for Web
        #[arg(long)]
        web: bool,

        /// Export for macOS
        #[arg(long)]
        macos: bool,

        /// Export for iOS
        #[arg(long)]
        ios: bool,

        /// Export for Android
        #[arg(long)]
        android: bool,
    },

    /// Remove export templates for a version
    Remove {
        /// Godot version to remove templates for
        #[arg(value_name = "GODOT_VERSION")]
        godot_version: String,

        /// Remove Windows templates only
        #[arg(long)]
        windows: bool,

        /// Remove Linux templates only
        #[arg(long)]
        linux: bool,

        /// Remove Web templates only
        #[arg(long)]
        web: bool,

        /// Remove macOS templates only
        #[arg(long)]
        macos: bool,

        /// Remove iOS templates only
        #[arg(long)]
        ios: bool,

        /// Remove Android templates only
        #[arg(long)]
        android: bool,
    },
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
            // No subcommand - detect and open project
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
            Commands::Recent => {
                commands::recent::run(&mut config)?;
            }
            Commands::Projects => {
                commands::projects::run(&mut config)?;
            }
            Commands::New { name } => {
                commands::new::run(&name, &mut config)?;
            }
            Commands::Build { windows, linux, web, macos, ios, android, debug } => {
                commands::build::run(windows, linux, web, macos, ios, android, debug, &mut config)?;
            }
            Commands::Up { setup, windows, linux, web, macos, ios, android, debug, name } => {
                commands::up::run(setup, windows, linux, web, macos, ios, android, debug, name, &mut config)?;
            }
            Commands::Uninstall { keep } => {
                commands::uninstall::run(keep)?;
            }
            Commands::Cost => {
                commands::cost::run(&config)?;
            }
            Commands::Templates { action } => match action {
                None | Some(TemplatesAction::List) => {
                    commands::templates::run_list(&config)?;
                }
                Some(TemplatesAction::Add { godot_version, windows, linux, web, macos, ios, android }) => {
                    commands::templates::run_add(&godot_version, windows, linux, web, macos, ios, android, &mut config)?;
                }
                Some(TemplatesAction::Remove { godot_version, windows, linux, web, macos, ios, android }) => {
                    commands::templates::run_remove(&godot_version, windows, linux, web, macos, ios, android, &mut config)?;
                }
            },
        },
    }

    Ok(())
}
