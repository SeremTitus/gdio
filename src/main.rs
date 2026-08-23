mod commands;
mod config;
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
        #[arg(short, long)]
        debug: bool,
    },

    /// Upload project to itch.io via butler
    Up {
        /// Run interactive setup for itch.io upload configuration
        #[arg(short, long)]
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

        /// Test folder (relative path, e.g. "test" → res://test/)
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
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

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
            Commands::Add {
                target,
                path,
                csharp,
            } => {
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
                commands::game::run(&config)?;
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
            Commands::Build {
                windows,
                linux,
                web,
                macos,
                ios,
                android,
                debug,
            } => {
                commands::build::run(windows, linux, web, macos, ios, android, debug, &config)?;
            }
            Commands::Up {
                setup,
                windows,
                linux,
                web,
                macos,
                ios,
                android,
                debug,
                name,
            } => {
                commands::up::run(
                    setup,
                    windows,
                    linux,
                    web,
                    macos,
                    ios,
                    android,
                    debug,
                    name,
                    &mut config,
                )?;
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
                commands::test::run(init, visual, folder.as_deref(), &mut config)?;
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
                    commands::addons::add::run(&mut config, &identifier, linked, select)?;
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
                    )?;
                }
                Some(AddonsAction::Exclude { identifier, revert }) => {
                    let identifier = identifier.map(|s| s.to_lowercase());
                    commands::addons::exclude::run(&mut config, identifier.as_deref(), revert)?;
                }
                Some(AddonsAction::Sync) => {
                    let cwd = std::env::current_dir()?;
                    let rt = tokio::runtime::Runtime::new()?;
                    commands::addons::sync::run(&mut config, &cwd, &rt)?;
                }
                Some(AddonsAction::Repository { url }) => {
                    commands::addons::repository::run(&mut config, url.as_deref())?;
                }
            },
            Commands::Templates { action } => match action {
                None | Some(TemplatesAction::List) => {
                    commands::templates::list::run(&config)?;
                }
                Some(TemplatesAction::Add {
                    godot_version,
                    windows,
                    linux,
                    web,
                    macos,
                    ios,
                    android,
                }) => {
                    commands::templates::add::run(
                        &godot_version,
                        windows,
                        linux,
                        web,
                        macos,
                        ios,
                        android,
                        &mut config,
                    )?;
                }
                Some(TemplatesAction::Remove {
                    godot_version,
                    windows,
                    linux,
                    web,
                    macos,
                    ios,
                    android,
                }) => {
                    commands::templates::remove::run(
                        &godot_version,
                        windows,
                        linux,
                        web,
                        macos,
                        ios,
                        android,
                        &mut config,
                    )?;
                }
            },
        },
    }

    Ok(())
}
