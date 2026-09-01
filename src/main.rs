mod commands;
mod config;
mod gdre;
mod github;
mod godot;
mod platform;
mod project;

use clap::{CommandFactory, Parser, Subcommand};
use platform::PlatformFlags;
use std::path::{Path, PathBuf};

fn detect_shell() -> clap_complete::Shell {
    // On all platforms, $SHELL is the most reliable indicator
    if let Ok(shell) = std::env::var("SHELL") {
        let shell_name = Path::new(&shell)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        match shell_name {
            "zsh" => return clap_complete::Shell::Zsh,
            "fish" => return clap_complete::Shell::Fish,
            "bash" | "sh" => return clap_complete::Shell::Bash,
            _ => {}
        }
    }

    if cfg!(windows) {
        clap_complete::Shell::PowerShell
    } else {
        clap_complete::Shell::Bash
    }
}

fn completions_install_path(shell: clap_complete::Shell) -> Option<PathBuf> {
    let home = dirs::home_dir()?;

    match shell {
        clap_complete::Shell::Bash => {
            if cfg!(target_os = "macos") {
                // Try Homebrew path first
                if let Ok(output) = std::process::Command::new("brew").arg("--prefix").output() {
                    if output.status.success() {
                        let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
                        let path = PathBuf::from(format!("{prefix}/etc/bash_completion.d/gdio"));
                        return Some(path);
                    }
                }
                Some(home.join(".local/share/bash-completion/completions/gdio"))
            } else {
                // Linux / other Unix
                if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
                    Some(PathBuf::from(format!(
                        "{xdg}/bash-completion/completions/gdio"
                    )))
                } else {
                    Some(home.join(".local/share/bash-completion/completions/gdio"))
                }
            }
        }
        clap_complete::Shell::Zsh => {
            if let Ok(zdotdir) = std::env::var("ZDOTDIR") {
                Some(PathBuf::from(format!("{zdotdir}/.zsh/completions/_gdio")))
            } else {
                Some(home.join(".zsh/completions/_gdio"))
            }
        }
        clap_complete::Shell::Fish => {
            if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
                Some(PathBuf::from(format!("{xdg}/fish/completions/gdio.fish")))
            } else {
                Some(home.join(".config/fish/completions/gdio.fish"))
            }
        }
        clap_complete::Shell::PowerShell => {
            if cfg!(windows) {
                Some(home.join("Documents/PowerShell/Modules/gdio/gdio.psm1"))
            } else {
                // Linux/macOS PowerShell Core
                if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
                    Some(PathBuf::from(format!(
                        "{xdg}/powershell/Modules/gdio/gdio.psm1"
                    )))
                } else {
                    Some(home.join(".config/powershell/Modules/gdio/gdio.psm1"))
                }
            }
        }
        _ => None,
    }
}

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

    /// Run the editor with custom arguments
    Args {
        /// Arguments to pass to the editor
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Run a script headless with the project editor
    Script {
        /// Script file path (e.g., my_script.gd)
        file: String,
    },

    /// Open the last project opened by gdio
    Recent,

    /// List and open projects known to gdio
    Projects,

    /// Create a new Godot project
    New {
        /// Project name (creates subdirectory in current dir)
        name: String,
    },

    /// Clone a Godot project from a git repository and open it
    Clone {
        /// Git repository URL
        url: String,

        /// Directory name (extracted from URL if omitted)
        dir: Option<String>,

        /// Shallow clone with given depth
        #[arg(short, long)]
        depth: Option<u32>,
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
        #[command(subcommand)]
        action: Option<UpAction>,

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

    /// Generate and install shell completions
    Completions {
        /// Shell to generate completions for (auto-detected if omitted with --install)
        shell: Option<clap_complete::Shell>,

        /// Install completions to the appropriate shell config directory
        #[arg(short, long)]
        install: bool,
    },

    /// Recover a Godot project from exported game files using GDRE Tools
    Recovery {
        /// Output path for recovered content (default: "recovered" folder in current dir)
        output: Option<String>,
    },

    /// List or toggle project tags
    Tags {
        /// Tag name to toggle (add if missing, remove if present)
        tag: Option<String>,
    },

    /// Show latest Godot news
    News {
        /// Number of articles to list (default: 5)
        count: Option<usize>,
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

        /// Download C# (mono) export templates
        #[arg(short, long)]
        csharp: bool,

        /// Download the debug only export templates
        #[arg(short, long)]
        debug: bool,

        /// Download the release only export templates
        #[arg(short, long)]
        release: bool,
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
enum UpAction {
    /// Setup itch.io upload configuration (game identifier)
    Setup {
        /// itch.io game identifier (e.g., myuser/mygame)
        game: String,
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

    if let Some(Commands::Completions { shell, install }) = cli.command {
        let shell = shell.unwrap_or_else(detect_shell);

        if install {
            let path = completions_install_path(shell)
                .ok_or_else(|| anyhow::anyhow!("unsupported shell for auto-install: {shell}"))?;

            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let mut file = std::fs::File::create(&path)?;
            clap_complete::generate(shell, &mut Cli::command(), "gdio", &mut file);

            println!("Installed {shell} completions to {}", path.display());
        } else {
            clap_complete::generate(shell, &mut Cli::command(), "gdio", &mut std::io::stdout());
        }

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
            Commands::Args { args } => {
                commands::args::run(&args, &config)?;
            }
            Commands::Script { file } => {
                commands::script::run(&file, &mut config).await?;
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
            Commands::Clone { url, dir, depth } => {
                commands::clone::run(&url, dir.as_deref(), depth, &mut config).await?;
            }
            Commands::Build { platform, debug } => {
                commands::build::run(&platform, debug, &config).await?;
            }
            Commands::Up {
                action,
                platform,
                debug,
                name,
            } => match action {
                None => {
                    commands::up::run(&platform, debug, name, &mut config).await?;
                }
                Some(UpAction::Setup { game }) => {
                    commands::up::run_setup_with_game(&game, &mut config).await?;
                }
            },
            Commands::Uninstall { keep } => {
                commands::uninstall::run(keep)?;
            }
            Commands::Cost => {
                commands::cost::run(&config)?;
            }
            Commands::Tags { tag } => {
                commands::tags::run(tag.as_deref())?;
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
                    csharp,
                    debug,
                    release,
                }) => {
                    commands::templates::add::run(
                        &godot_version,
                        &platform,
                        csharp,
                        debug,
                        release,
                        &mut config,
                    )
                    .await?;
                }
                Some(TemplatesAction::Remove {
                    godot_version,
                    platform,
                }) => {
                    commands::templates::remove::run(&godot_version, &platform, &mut config)?;
                }
            },
            Commands::Completions { .. } => unreachable!(),

            Commands::Recovery { output } => {
                commands::recovery::run(output.as_deref(), &mut config).await?;
            }
            Commands::News { count } => {
                commands::news::run(count.unwrap_or(5)).await?;
                return Ok(());
            }
        },
    }

    let _ = commands::news::show_latest(&mut config).await;

    Ok(())
}
