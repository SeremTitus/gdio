mod commands;
mod config;
mod github;
mod godot;
mod platform;
mod project;

use clap::{CommandFactory, Parser, Subcommand};
use platform::PlatformFlags;

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
        path: Option<String>,

        /// Download the C# (mono) variant
        #[arg(short, long)]
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
        #[command(flatten)]
        platform: PlatformFlags,

        /// Export in debug mode
        #[arg(short, long)]
        debug: bool,
    },

    /// Upload project to itch.io via butler
    Up {
        /// Run interactive setup for itch.io upload configuration
        #[arg(short, long)]
        setup: bool,

        #[command(flatten)]
        platform: PlatformFlags,

        /// Export in debug mode before upload
        #[arg(short, long)]
        debug: bool,

        /// Interactively name zip files before upload
        #[arg(short, long)]
        name: bool,
    },

    /// Uninstall gdio and remove all its files
    Uninstall {
        /// Keep config and editor binaries
        #[arg(short, long)]
        keep: bool,
    },

    /// Show disk space used by gdio
    Cost,

    /// Run GUT tests
    Test {
        /// Install GUT addon
        #[arg(short, long)]
        init: bool,

        /// Open GUT test runner window (no headless)
        #[arg(short, long)]
        visual: bool,

        /// Test folder (relative path, e.g. "test" -> res://test/)
        folder: Option<String>,
    },

    /// Manage addons from the asset library
    Addons {
        #[command(subcommand)]
        action: Option<AddonsAction>,
    },

    /// Manage export templates
    Templates {
        #[command(subcommand)]
        action: Option<TemplatesAction>,
    },

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: clap_complete::Shell,
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

        #[command(flatten)]
        platform: PlatformFlags,
    },

    /// Remove export templates for a version
    Remove {
        /// Godot version to remove templates for
        #[arg(value_name = "GODOT_VERSION")]
        godot_version: String,

        #[command(flatten)]
        platform: PlatformFlags,
    },
}

#[derive(Subcommand)]
#[command(rename_all = "lowercase")]
enum AddonsAction {
    /// Install an addon (identifier: publisher/asset)
    Add {
        /// Addon identifier in format publisher/asset (e.g., bitwes/gut)
        identifier: String,

        /// Link addon globally (stored in gdio config, symlinked into project)
        #[arg(short, long)]
        linked: bool,

        /// Interactively select which version to install
        #[arg(short, long)]
        select: bool,
    },

    /// List addons
    List {
        /// List linked addons
        #[arg(short, long)]
        linked: bool,
    },

    /// Remove addon(s) by folder name (interactive if no arguments)
    Remove {
        /// Folder name(s) or identifier(s) of addons to remove
        identifiers: Vec<String>,
    },

    /// Manage global addons (synced to all projects unless excluded)
    Globals {
        /// Addon identifier to add as global
        identifier: Option<String>,

        /// Interactively remove a global addon (stops syncing to new projects)
        #[arg(short, long)]
        remove: bool,

        /// Interactively select which version to install
        #[arg(short, long)]
        select: bool,

        /// Store in global cache and symlink into projects (instead of copying)
        #[arg(short, long)]
        linked: bool,
    },

    /// Manage project exclusions for global addons
    Exclude {
        /// Addon identifier to exclude from this project
        identifier: Option<String>,

        /// Revert the exclusion (re-add this project to the addon's sync list)
        #[arg(short, long)]
        revert: bool,
    },

    /// Sync linked and global addons
    Sync,

    /// Manage addon repositories (list or toggle add/remove by URL)
    Repository {
        /// Repository URL (omit to list, provide to toggle add/remove)
        url: Option<String>,
    },

    /// Search the asset store by name/description
    Search {
        /// Search query
        query: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.version {
        println!("gdio {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if let Some(Commands::Completions { shell }) = cli.command {
        clap_complete::generate(shell, &mut Cli::command(), "gdio", &mut std::io::stdout());
        return Ok(());
    }

    let mut config = config::Config::load()?;

    if cli.recent {
        commands::recent::run(&mut config).await?;
        return Ok(());
    }

    match cli.command {
        None => {
            // No subcommand - detect and open project
            commands::default::run(&mut config).await?;
        }
        Some(cmd) => match cmd {
            Commands::Add {
                target,
                path,
                csharp,
            } => {
                commands::add::run(&target, path.as_deref(), csharp, &mut config).await?;
            }
            Commands::List => {
                commands::list::run(&config)?;
            }
            Commands::Remove { target } => {
                commands::remove::run(target.as_deref(), &mut config)?;
            }
            Commands::Bind { target } => {
                commands::bind::run(target.as_deref(), &mut config).await?;
            }
            Commands::Game => {
                commands::game::run(&config)?;
            }
            Commands::Recent => {
                commands::recent::run(&mut config).await?;
            }
            Commands::Projects => {
                commands::projects::run(&mut config).await?;
            }
            Commands::New { name } => {
                commands::new::run(&name, &mut config).await?;
            }
            Commands::Build { platform, debug } => {
                commands::build::run(&platform, debug, &config).await?;
            }
            Commands::Up {
                setup,
                platform,
                debug,
                name,
            } => {
                commands::up::run(setup, &platform, debug, name, &mut config)?;
            }
            Commands::Uninstall { keep } => {
                commands::uninstall::run(keep)?;
            }
            Commands::Cost => {
                commands::cost::run(&config)?;
            }
            Commands::Test {
                init,
                visual,
                folder,
            } => {
                commands::test::run(init, visual, folder.as_deref(), &mut config).await?;
            }
            Commands::Addons { action } => match action {
                None => {
                    commands::addons::list::run(&config, false)?;
                }
                Some(AddonsAction::List { linked }) => {
                    commands::addons::list::run(&config, linked)?;
                }
                Some(AddonsAction::Add {
                    identifier,
                    linked,
                    select,
                }) => {
                    let identifier = identifier.to_lowercase();
                    commands::addons::add::run(&mut config, &identifier, linked, select).await?;
                }
                Some(AddonsAction::Remove { identifiers }) => {
                    let identifiers: Vec<String> =
                        identifiers.iter().map(|s| s.to_lowercase()).collect();
                    commands::addons::remove::run(&mut config, &identifiers)?;
                }
                Some(AddonsAction::Globals {
                    identifier,
                    remove,
                    select,
                    linked,
                }) => {
                    let identifier = identifier.map(|s| s.to_lowercase());
                    commands::addons::globals::run(
                        &mut config,
                        identifier.as_deref(),
                        remove,
                        select,
                        linked,
                    )
                    .await?;
                }
                Some(AddonsAction::Exclude { identifier, revert }) => {
                    let identifier = identifier.map(|s| s.to_lowercase());
                    commands::addons::exclude::run(&mut config, identifier.as_deref(), revert)?;
                }
                Some(AddonsAction::Sync) => {
                    let ctx = commands::shared::ProjectContext::detect("Unknown Project")?;
                    commands::addons::sync::run(&mut config, &ctx.cwd).await?;
                }
                Some(AddonsAction::Repository { url }) => {
                    commands::addons::repository::run(&mut config, url.as_deref())?;
                }
                Some(AddonsAction::Search { query }) => {
                    commands::addons::search::run(&query, &config).await?;
                }
            },
            Commands::Templates { action } => match action {
                None | Some(TemplatesAction::List) => {
                    commands::templates::list::run(&config)?;
                }
                Some(TemplatesAction::Add {
                    godot_version,
                    platform,
                }) => {
                    commands::templates::add::run(&godot_version, &platform, &mut config).await?;
                }
                Some(TemplatesAction::Remove {
                    godot_version,
                    platform,
                }) => {
                    commands::templates::remove::run(&godot_version, &platform, &mut config)?;
                }
            },
            Commands::Completions { .. } => unreachable!(),
        },
    }

    Ok(())
}
