use super::api;
use super::storage;
use crate::config::{Config, LinkedAddonInfo};
use anyhow::Result;

/// Downloads and installs an addon from a Godot Asset Store repository.
///
/// # Arguments
/// * `identifier` - Addon identifier in format `publisher/asset` (e.g. `bitwes/gut`)
/// * `linked` - If true, store in global addon cache and symlink into project
/// * `select` - If true, interactively select which version to install
///
/// # Installation modes
/// - **Local** (default): Extract to `{project}/addons/`
/// - **Linked** (`--linked`): Extract to global store, symlink into project, update .gdio
pub async fn run(config: &mut Config, identifier: &str, linked: bool, select: bool) -> Result<()> {
    // Parse the identifier into publisher/asset components
    let (publisher, asset) = parse_identifier(identifier)?;

    let ctx = crate::commands::shared::ProjectContext::detect("Unknown Project")?;

    // Get the bound Godot version for compatibility filtering
    let project_key = ctx.project_path.clone();
    let godot_version = match config
        .projects
        .get(&project_key)
        .and_then(|p| p.bound_editor.clone())
    {
        Some(v) => v,
        None => {
            anyhow::bail!("No Godot version bound to this project. Run `gdio bind` first.");
        }
    };

    println!("Fetching releases for {}...", identifier);

    // Set up async runtime and HTTP client
    let client = reqwest::Client::builder().user_agent("gdio").build()?;

    // Collect all compatible releases from all repositories
    let mut all_releases: Vec<(String, String, api::Release)> = Vec::new();
    let mut not_found = false;

    for repo in &config.addons.repositories {
        match api::fetch_releases(&client, &repo.url, publisher, asset).await {
            Ok(releases) => {
                let compatible = api::list_compatible_releases(&releases, &godot_version);
                for r in compatible {
                    all_releases.push((repo.name.clone(), repo.url.clone(), r.clone()));
                }
            }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("does not exist") {
                    not_found = true;
                    break;
                }
                println!("  Skipping {}: {}", repo.name, e);
            }
        }
    }

    if not_found {
        anyhow::bail!("Addon '{}' does not exist on the asset store.", identifier);
    }

    // Pick a release — interactive selection or best match
    let (version, download_url, used_repo) = if select {
        // Interactive selection: show all compatible versions
        if all_releases.is_empty() {
            anyhow::bail!("No compatible releases found for {}", identifier);
        }
        let items: Vec<String> = all_releases
            .iter()
            .map(|(repo_name, _, release)| {
                let marker = if release.stable { "" } else { " (pre-release)" };
                format!("v{} [{}]{}", release.version, repo_name, marker)
            })
            .collect();
        crate::commands::shared::require_interactive(
            "Version selection required. Run `gdio addon add` interactively to choose a version.",
        )?;
        let idx = dialoguer::Select::new()
            .with_prompt(format!("Select version for {}", identifier))
            .items(&items)
            .default(0)
            .interact()?;
        let picked = &all_releases[idx];
        println!("  Selected v{}", picked.2.version);
        (
            picked.2.version.clone(),
            picked.2.download_url.clone(),
            picked.1.clone(),
        )
    } else {
        // Automatic selection: pick the best release
        match all_releases.first() {
            Some((repo_name, repo_url, release)) => {
                println!("  Found v{} from {}", release.version, repo_name);
                (
                    release.version.clone(),
                    release.download_url.clone(),
                    repo_url.clone(),
                )
            }
            None => {
                anyhow::bail!("No compatible release found for {}", identifier);
            }
        }
    };

    // Detect current installation mode and handle switching
    let gdio = storage::read_gdio(&ctx.cwd);
    let currently_linked = gdio.addons.contains_key(identifier);
    let current_folder = config
        .addons
        .linked
        .get(identifier)
        .map(|g| g.folder_name.clone())
        .or_else(|| storage::find_local_addon_folder(&ctx.cwd, asset));

    if currently_linked && !linked {
        // Switching linked → local: remove the symlink
        if let Some(ref folder) = current_folder {
            let symlink_path = ctx.cwd.join("addons").join(folder);
            if symlink_path.exists() {
                storage::remove_symlink(&symlink_path)?;
                println!("Removed linked symlink: {}", folder);
            }
        }
        // Remove from .gdio and .gitignore
        if let Some(ref folder) = current_folder {
            storage::remove_linked(&ctx.cwd, identifier, folder)?;
        }
        // Remove project from linked reference list
        let should_remove_entry = if let Some(info) = config.addons.linked.get_mut(identifier) {
            info.projects.retain(|p| p != &project_key);
            info.projects.is_empty()
        } else {
            false
        };
        if should_remove_entry {
            let info = config.addons.linked.remove(identifier).unwrap();
            storage::cleanup_global_store(identifier, Some(&info.version));
            println!(
                "Removed {} from global store (no projects linked)",
                identifier
            );
        }
    } else if !currently_linked && linked && current_folder.is_some() {
        // Switching local → linked: remove the local directory
        if let Some(ref folder) = current_folder {
            let local_path = ctx.cwd.join("addons").join(folder);
            if local_path.exists() && !storage::is_symlink(&local_path) {
                std::fs::remove_dir_all(&local_path)?;
                println!("Removed local install: {}", folder);
            }
        }
    }

    // Check if this version already exists in the global store (cache hit)
    let global_dir = Config::get_global_addons_dir();
    let addon_global_dir = global_dir.join(format!("{}_{}_{}", publisher, asset, version));
    let cached = addon_global_dir.exists()
        && addon_global_dir
            .read_dir()
            .map(|mut dirs| dirs.next().is_some())
            .unwrap_or(false);

    // Install based on the mode (linked or local)
    if linked {
        // --- LINKED INSTALL ---
        // Store path includes version so multiple versions can coexist
        let folder_name = if cached {
            // Cache hit — find the addon folder name from the store
            let name = find_folder_name_in_store(&addon_global_dir)?;
            println!("Using cached {} v{} ({})", identifier, version, name);
            name
        } else {
            // Cache miss — download and extract to global store
            let cache_dir = Config::get_addons_cache_dir();
            let zip_name = format!("{}_v{}.zip", identifier.replace('/', "_"), version);
            println!("Downloading {} v{}...", identifier, version);
            let zip_path = api::download_zip(&client, &download_url, &cache_dir, &zip_name).await?;
            println!("Extracting to global store...");
            let name = storage::extract_addon(&zip_path, &addon_global_dir, true)?;
            let _ = std::fs::remove_file(&zip_path);
            name
        };

        // Create the addons/ directory and symlink
        let project_addons = ctx.cwd.join("addons").join(&folder_name);
        std::fs::create_dir_all(ctx.cwd.join("addons"))?;

        // Create symlink: {project}/addons/{folder_name} -> {global_store}/{publisher}_{asset}_{version}/{folder_name}
        storage::create_symlink(&addon_global_dir.join(&folder_name), &project_addons)?;

        // Update .gdio and .gitignore
        storage::add_linked(&ctx.cwd, identifier, &version, &used_repo, &folder_name)?;

        // Register in the linked config for cross-project awareness
        let project_key = ctx.cwd.to_string_lossy().to_string();
        let entry = config
            .addons
            .linked
            .entry(identifier.to_string())
            .or_insert_with(|| LinkedAddonInfo {
                version: version.clone(),
                folder_name: folder_name.clone(),
                projects: Vec::new(),
            });
        if !entry.projects.contains(&project_key) {
            entry.projects.push(project_key);
        }
        entry.version = version.clone();
        entry.folder_name = folder_name.clone();

        println!("Installed {} v{} (linked)", identifier, version);
        storage::enable_plugin(&ctx.cwd, &folder_name)?;
    } else {
        // --- LOCAL INSTALL ---
        let folder_name = if cached {
            let name = find_folder_name_in_store(&addon_global_dir)?;
            println!("Using cached {} v{} ({})", identifier, version, name);
            // Copy from global store to project
            let src = addon_global_dir.join(&name);
            let dst = ctx.cwd.join("addons").join(&name);
            if !dst.exists() {
                std::fs::create_dir_all(dst.parent().unwrap())?;
                storage::copy_dir_all(&src, &dst)?;
            }
            name
        } else {
            let cache_dir = Config::get_addons_cache_dir();
            let zip_name = format!("{}_v{}.zip", identifier.replace('/', "_"), version);
            println!("Downloading {} v{}...", identifier, version);
            let zip_path = api::download_zip(&client, &download_url, &cache_dir, &zip_name).await?;
            println!("Extracting to project...");
            let name = storage::extract_addon(&zip_path, &ctx.cwd, false)?;
            let _ = std::fs::remove_file(&zip_path);
            name
        };

        println!("Installed {} v{} as {}", identifier, version, folder_name);
        storage::enable_plugin(&ctx.cwd, &folder_name)?;
    }

    // Persist config changes
    config.save()?;
    Ok(())
}

/// Parse a `publisher/asset` identifier string into its two components.
///
/// Returns an error if the format is invalid (missing `/`, empty parts).
fn parse_identifier(identifier: &str) -> Result<(&str, &str)> {
    let parts: Vec<&str> = identifier.splitn(2, '/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        anyhow::bail!(
            "Invalid identifier '{}'. Expected format: publisher/asset (e.g., bitwes/gut)",
            identifier
        );
    }
    Ok((parts[0], parts[1]))
}

/// Find the addon folder name inside a global store directory.
///
/// Scans for a directory containing `plugin.cfg` (the Godot addon marker).
fn find_folder_name_in_store(store_dir: &std::path::Path) -> Result<String> {
    for entry in std::fs::read_dir(store_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() && entry.path().join("plugin.cfg").exists() {
            return Ok(entry.file_name().to_string_lossy().to_string());
        }
    }
    anyhow::bail!("No addon folder found in {}", store_dir.display())
}
