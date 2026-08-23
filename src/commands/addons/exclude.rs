use super::storage;
use crate::config::Config;
use anyhow::{Context, Result};

/// Manages project exclusions for global addons.
///
/// # Arguments
/// * `identifier` - Addon identifier to exclude (or revert exclusion for)
/// * `revert` - If true, revert the exclusion (re-add this project to sync list)
pub fn run(config: &mut Config, identifier: Option<&str>, revert: bool) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let project_file = cwd.join("project.godot");

    if !project_file.exists() {
        anyhow::bail!("Not in a Godot project directory.");
    }

    let project_path = cwd.to_string_lossy().to_string();

    if revert {
        // Revert exclusion: remove project from the exclusion list and re-add addon
        let ident = match identifier {
            Some(id) => id.to_string(),
            None => {
                // Interactive: select from excluded addons
                return run_revert_interactive(config, &project_path);
            }
        };

        return revert_exclusion(config, &ident, &project_path, &cwd);
    }

    // Exclude: add project to the exclusion list and remove addon from project
    let ident = match identifier {
        Some(id) => id.to_string(),
        None => {
            // Interactive: select from global addons
            return run_exclude_interactive(config, &project_path);
        }
    };

    if !config.addons.globals.contains_key(&ident) {
        anyhow::bail!("'{}' is not a global addon.", ident);
    }

    let exclusions = config
        .addons
        .globals_exclusions
        .entry(ident.clone())
        .or_default();

    if exclusions.contains(&project_path) {
        println!("{} is already excluded for {}", project_path, ident);
        return Ok(());
    }

    exclusions.push(project_path.clone());
    println!("Excluded {} from {}", project_path, ident);

    // Remove the addon from the project
    if let Some(global_info) = config.addons.globals.get(&ident) {
        let folder_name = &global_info.folder_name;
        let addon_path = cwd.join("addons").join(folder_name);

        if addon_path.exists() || addon_path.symlink_metadata().is_ok() {
            let is_link = storage::is_symlink(&addon_path);
            if is_link {
                storage::remove_symlink(&addon_path)?;
                println!("  removed symlink: {}", folder_name);
            } else if addon_path.is_dir() {
                std::fs::remove_dir_all(&addon_path)?;
                println!("  removed: {}", folder_name);
            }

            // Clean up .gitignore
            crate::commands::addons::storage::remove_linked(&cwd, "", folder_name)?;
        }
    }

    config.save()?;
    Ok(())
}

/// Revert an exclusion: remove from exclusion list and re-add addon to the project.
fn revert_exclusion(
    config: &mut Config,
    ident: &str,
    project_path: &str,
    project_dir: &std::path::Path,
) -> Result<()> {
    if let Some(exclusions) = config.addons.globals_exclusions.get_mut(ident) {
        let before = exclusions.len();
        exclusions.retain(|p| p != project_path);
        if exclusions.len() < before {
            println!("Removed {} from exclusion list for {}", project_path, ident);
        } else {
            println!("{} was not excluded for {}", project_path, ident);
            return Ok(());
        }
        if exclusions.is_empty() {
            config.addons.globals_exclusions.remove(ident);
        }
    } else {
        println!("{} is not excluded for any addon", project_path);
        return Ok(());
    }

    // Re-add the addon to the project
    let addons_dir = project_dir.join("addons");
    if let Some(global_info) = config.addons.globals.get(ident) {
        let folder_name = &global_info.folder_name;
        let addon_path = addons_dir.join(folder_name);

        if !addon_path.exists() && addon_path.symlink_metadata().is_err() {
            let global_dir = Config::get_global_addons_dir();
            let parts: Vec<&str> = ident.splitn(2, '/').collect();
            if parts.len() == 2 {
                let linked_info = config.addons.linked.get(ident);
                let version = linked_info.map(|g| g.version.as_str()).unwrap_or("unknown");
                let global_addon_dir =
                    global_dir.join(format!("{}_{}_{}", parts[0], parts[1], version));
                let global_addon_content_dir = global_addon_dir.join(folder_name);

                if global_addon_content_dir.exists() {
                    std::fs::create_dir_all(&addons_dir)?;
                    if config.addons.linked.contains_key(ident) {
                        storage::create_symlink(&global_addon_content_dir, &addon_path)?;
                        println!("  re-added (symlink): {}", folder_name);
                    } else {
                        storage::copy_dir_all(&global_addon_content_dir, &addon_path)?;
                        println!("  re-added (copied): {}", folder_name);
                    }
                } else {
                    println!("  addon not in global store, will sync later");
                }
            }
        } else {
            println!("  addon already exists in project");
        }
    }

    config.save()?;
    Ok(())
}

/// Interactive selection to exclude a global addon.
fn run_exclude_interactive(config: &mut Config, project_path: &str) -> Result<()> {
    let items: Vec<String> = config
        .addons
        .globals
        .iter()
        .map(|(ident, info)| {
            let excluded = config
                .addons
                .globals_exclusions
                .get(ident)
                .map(|e| e.contains(&project_path.to_string()))
                .unwrap_or(false);
            if excluded {
                format!(
                    "{} v{} (already excluded)",
                    ident,
                    info.version.as_deref().unwrap_or("latest")
                )
            } else {
                format!("{} v{}", ident, info.version.as_deref().unwrap_or("latest"))
            }
        })
        .collect();

    if items.is_empty() {
        println!("No global addons registered.");
        return Ok(());
    }

    let idx = dialoguer::Select::new()
        .with_prompt("Select addon to exclude from this project")
        .items(&items)
        .interact()?;

    let identifier = config.addons.globals.keys().nth(idx).unwrap().clone();

    let exclusions = config
        .addons
        .globals_exclusions
        .entry(identifier.clone())
        .or_default();

    if exclusions.contains(&project_path.to_string()) {
        println!("{} is already excluded for {}", project_path, identifier);
        return Ok(());
    }

    exclusions.push(project_path.to_string());
    println!("Excluded {} from {}", project_path, identifier);

    // Remove the addon from the project (same as non-interactive path)
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    if let Some(global_info) = config.addons.globals.get(&identifier) {
        let folder_name = &global_info.folder_name;
        let addon_path = cwd.join("addons").join(folder_name);

        if addon_path.exists() || addon_path.symlink_metadata().is_ok() {
            let is_link = storage::is_symlink(&addon_path);
            if is_link {
                storage::remove_symlink(&addon_path)?;
                println!("  removed symlink: {}", folder_name);
            } else if addon_path.is_dir() {
                std::fs::remove_dir_all(&addon_path)?;
                println!("  removed: {}", folder_name);
            }

            crate::commands::addons::storage::remove_linked(&cwd, "", folder_name)?;
        }
    }

    config.save()?;
    Ok(())
}

/// Interactive selection to revert an exclusion.
fn run_revert_interactive(config: &mut Config, project_path: &str) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;

    let excluded: Vec<(String, String)> = config
        .addons
        .globals_exclusions
        .iter()
        .filter(|(_, exclusions)| exclusions.contains(&project_path.to_string()))
        .filter_map(|(ident, _)| {
            let version = config
                .addons
                .globals
                .get(ident)?
                .version
                .clone()
                .unwrap_or_else(|| "latest".to_string());
            Some((ident.clone(), version))
        })
        .collect();

    if excluded.is_empty() {
        println!("{} is not excluded from any global addon", project_path);
        return Ok(());
    }

    let items: Vec<String> = excluded
        .iter()
        .map(|(ident, ver)| format!("{} v{}", ident, ver))
        .collect();

    let idx = dialoguer::Select::new()
        .with_prompt("Select addon to un-exclude from this project")
        .items(&items)
        .interact()?;

    let identifier = &excluded[idx].0;
    revert_exclusion(config, identifier, project_path, &cwd)
}
