use super::{api, storage};
use crate::config::{Config, GlobalAddonEntry};
use anyhow::{Context, Result};

/// Manages global addons that are synced to all projects unless excluded.
///
/// # Arguments
/// * `identifier` - Addon identifier (for add or remove)
/// * `remove` - If true, remove a global addon (interactive if no identifier)
/// * `select` - If true, interactively select which version to install
/// * `linked` - If true, store in global cache and symlink into projects
pub async fn run(
    config: &mut Config,
    identifier: Option<&str>,
    remove: bool,
    select: bool,
    linked: bool,
) -> Result<()> {
    if remove {
        return match identifier {
            Some(ident) => run_remove_one(config, ident),
            None => run_remove(config),
        };
    }

    if let Some(ident) = identifier {
        return run_add(config, ident, select, linked).await;
    }

    // List global addons
    if config.addons.globals.is_empty() {
        println!("No global addons registered.");
        return Ok(());
    }

    println!("Global addons:");
    for (ident, info) in &config.addons.globals {
        let excluded_count = config
            .addons
            .globals_exclusions
            .get(ident)
            .map(|e| e.len())
            .unwrap_or(0);
        let excluded_note = if excluded_count > 0 {
            format!(" ({} excluded)", excluded_count)
        } else {
            String::new()
        };
        println!(
            "  {: <30} v{:<10} {}{}",
            ident,
            info.version.as_deref().unwrap_or("latest"),
            info.folder_name,
            excluded_note
        );
    }

    Ok(())
}

/// Add an addon as a global addon (synced to all projects).
async fn run_add(config: &mut Config, identifier: &str, select: bool, linked: bool) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let in_project = cwd.join("project.godot").exists();

    // Check if already registered as global
    if config.addons.globals.contains_key(identifier) {
        println!("'{}' is already a global addon.", identifier);
        return Ok(());
    }

    let parts: Vec<&str> = identifier.splitn(2, '/').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid identifier format. Expected: publisher/asset");
    }
    let (publisher, asset) = (parts[0], parts[1]);

    // Try to determine folder_name and version from multiple sources
    let mut folder_name = None;
    let mut version = String::new();
    let mut repository = String::new();

    // 1. Try local files (if in project and addon exists locally)
    if in_project && let Some(name) = storage::find_local_addon_folder(&cwd, asset) {
        let addons_dir = cwd.join("addons");
        let plugin_cfg = addons_dir.join(&name).join("plugin.cfg");
        version = if plugin_cfg.exists() {
            parse_plugin_version(&plugin_cfg).unwrap_or_default()
        } else {
            String::new()
        };
        let gdio = storage::read_gdio(&cwd);
        repository = gdio
            .addons
            .get(identifier)
            .map(|e| e.repository.clone())
            .unwrap_or_default();
        folder_name = Some(name);
    }

    // 2. Try linked config / global store
    if folder_name.is_none() {
        if let Some(l) = config.addons.linked.get(identifier) {
            folder_name = Some(l.folder_name.clone());
            version = l.version.clone();
        } else {
            let global_dir = Config::get_global_addons_dir();
            folder_name = find_folder_in_global_store(&global_dir, publisher, asset);
        }
    }

    // 3. Fetch from API to verify addon exists and get version(s)
    let client = reqwest::Client::builder().user_agent("gdio").build()?;

    // Collect all releases from all repositories (no compatibility filter)
    let mut all_releases: Vec<(String, String, api::Release)> = Vec::new();
    for repo in &config.addons.repositories {
        match api::fetch_releases(&client, &repo.url, publisher, asset).await {
            Ok(releases) => {
                for r in &releases {
                    all_releases.push((repo.name.clone(), repo.url.clone(), r.clone()));
                }
            }
            Err(e) => eprintln!("  Skipping {}: {}", repo.name, e),
        }
    }

    // Select version — interactive or automatic
    if select {
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
        let idx = dialoguer::Select::new()
            .with_prompt(format!("Select version for {}", identifier))
            .items(&items)
            .default(0)
            .interact()?;
        let picked = &all_releases[idx];
        version = picked.2.version.clone();
        repository = picked.1.clone();
        println!("  Selected v{}", version);
    } else {
        // Automatic selection: pick the best release
        match all_releases.first() {
            Some((repo_name, repo_url, release)) => {
                version = release.version.clone();
                repository = repo_url.clone();
                println!("  Found v{} from {}", version, repo_name);
            }
            None => {
                if folder_name.is_none() || version.is_empty() {
                    anyhow::bail!("Addon '{}' does not exist on the asset store.", identifier);
                }
            }
        }
    }

    if folder_name.is_none() {
        folder_name = Some(asset.to_string());
    }

    let folder_name = folder_name.unwrap();

    config.addons.globals.insert(
        identifier.to_string(),
        GlobalAddonEntry {
            version: if select { Some(version) } else { None },
            folder_name: folder_name.clone(),
            repository,
            linked,
        },
    );

    println!("Added '{}' as a global addon.", identifier);

    // Sync to current project if we're in one
    if in_project {
        super::sync::run(config, &cwd).await?;
    } else {
        println!("  (will sync to projects during `gdio addons sync`)");
    }

    config.save()?;
    Ok(())
}

/// Remove a specific addon from the global list.
fn run_remove_one(config: &mut Config, identifier: &str) -> Result<()> {
    if !config.addons.globals.contains_key(identifier) {
        anyhow::bail!("'{}' is not a global addon.", identifier);
    }

    config.addons.globals.remove(identifier);
    println!("Removed {} from global addons", identifier);

    config.save()?;
    Ok(())
}

/// Interactive menu to remove a global addon (stops syncing to new projects).
fn run_remove(config: &mut Config) -> Result<()> {
    if config.addons.globals.is_empty() {
        println!("No global addons registered.");
        return Ok(());
    }

    let items: Vec<String> = config
        .addons
        .globals
        .iter()
        .map(|(ident, info)| format!("{} v{}", ident, info.version.as_deref().unwrap_or("latest")))
        .collect();

    let idx = dialoguer::Select::new()
        .with_prompt("Select global addon to stop syncing")
        .items(&items)
        .interact()?;

    let identifier = config.addons.globals.keys().nth(idx).unwrap().clone();
    config.addons.globals.remove(&identifier);

    println!("Removed {} from global addons list", identifier);

    config.save()?;
    Ok(())
}

/// Search the global store for a folder matching publisher/asset.
fn find_folder_in_global_store(
    global_dir: &std::path::Path,
    publisher: &str,
    asset: &str,
) -> Option<String> {
    let prefix = format!("{}_{}_", publisher, asset);
    for entry in std::fs::read_dir(global_dir).ok()? {
        let entry = entry.ok()?;
        if !entry.file_type().ok()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&prefix) {
            // Found a versioned dir like gut_bitwes_9.3.0 — check for addon folder inside
            let store_dir = entry.path();
            for inner in std::fs::read_dir(&store_dir).ok()? {
                let inner = inner.ok()?;
                if inner.file_type().ok()?.is_dir() && inner.path().join("plugin.cfg").exists() {
                    return Some(inner.file_name().to_string_lossy().to_string());
                }
            }
        }
    }
    None
}

/// Parse version from plugin.cfg
fn parse_plugin_version(path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("version=") {
            return trimmed
                .split_once('=')
                .map(|(_, v)| v.trim_matches('"').to_string());
        }
    }
    None
}
