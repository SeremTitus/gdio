use super::storage;
use crate::config::Config;
use anyhow::{Context, Result};

/// Removes addon(s) by folder name or identifier from the current project.
///
/// # Input formats
/// - **Folder name** (e.g. `gut`): removes `addons/gut/` directly
/// - **Identifier** (e.g. `seremtitus/ruzta`): looks up the folder name from config
///
/// # Behavior
/// - If the addon is a symlink: removes the symlink only (global store is preserved)
/// - If the addon is a directory: removes the entire directory
/// - Updates `.gdio` file if the addon was tracked as global
/// - Use `gdio addons exclude` to manage every-project exclusions
pub fn run(config: &mut Config, identifiers: &[String]) -> Result<()> {
    // Validate we're in a Godot project directory
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let project_file = cwd.join("project.godot");

    if !project_file.exists() {
        anyhow::bail!("Not in a Godot project directory.");
    }

    let addons_dir = cwd.join("addons");
    if !addons_dir.exists() {
        println!("No addons directory found.");
        return Ok(());
    }

    let project_key = cwd.to_string_lossy().to_string();

    // If no identifiers provided, do interactive selection
    let inputs: Vec<String> = if identifiers.is_empty() {
        // Scan addons/ for installed addons
        let mut entries: Vec<_> = std::fs::read_dir(&addons_dir)
            .context("Failed to read addons directory")?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();
        entries.sort_by_key(|e| e.file_name());

        if entries.is_empty() {
            println!("No addons installed.");
            return Ok(());
        }

        let items: Vec<String> = entries
            .iter()
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let is_link = storage::is_symlink(&e.path());
                if is_link {
                    format!("{} (global)", name)
                } else {
                    name
                }
            })
            .collect();

        let idx = dialoguer::Select::new()
            .with_prompt("Select addon to remove")
            .items(&items)
            .interact()?;

        vec![entries[idx].file_name().to_string_lossy().to_string()]
    } else {
        identifiers.to_vec()
    };

    // Process each identifier or folder name
    for input in &inputs {
        // Determine if this is an identifier (publisher/asset) or a plain folder name
        let (folder_name, identifier) = if input.contains('/') {
            // Identifier format: look up folder name from config
            let folder = resolve_folder_name(config, input);
            (
                folder.unwrap_or_else(|| input.split('/').next_back().unwrap_or(input).to_string()),
                Some(input.as_str()),
            )
        } else {
            // Plain folder name
            (input.clone(), None)
        };

        let addon_path = addons_dir.join(&folder_name);

        // Skip if the addon doesn't exist
        if !addon_path.exists() && addon_path.symlink_metadata().is_err() {
            println!("Addon '{}' not found, skipping.", folder_name);
            continue;
        }

        // Remove the addon (symlink or directory)
        let is_link = storage::is_symlink(&addon_path);
        if is_link {
            storage::remove_symlink(&addon_path)?;
            println!("Removed symlink: {}", folder_name);
        } else if addon_path.is_dir() {
            std::fs::remove_dir_all(&addon_path)?;
            println!("Removed: {}", folder_name);
        }

        // If identifier was provided, handle config tracking
        if let Some(ident) = identifier {
            // Remove from .gdio and .gitignore
            storage::remove_linked(&cwd, ident, &folder_name)?;
            // Remove project from linked reference list
            remove_project_reference(config, ident, &project_key);
        } else {
            // Plain folder name — try to find matching entries in .gdio
            let gdio = storage::read_gdio(&cwd);
            let matched_ident = gdio
                .addons
                .keys()
                .find(|k| k.split('/').next_back().unwrap_or(k) == folder_name.as_str())
                .or_else(|| {
                    config.addons.linked.iter().find_map(|(ident, info)| {
                        if info.folder_name == folder_name {
                            Some(ident)
                        } else {
                            None
                        }
                    })
                })
                .cloned();

            if let Some(ident) = matched_ident {
                storage::remove_linked(&cwd, &ident, &folder_name)?;
                remove_project_reference(config, &ident, &project_key);
            } else {
                // No matching identifier in .gdio, just clean up .gitignore
                storage::remove_linked(&cwd, "", &folder_name)?;
            }
        }
    }

    // Save config changes
    config.save()?;
    Ok(())
}

/// Look up the folder name for an identifier from linked config and .gdio file.
fn resolve_folder_name(config: &Config, identifier: &str) -> Option<String> {
    // Check linked config first
    if let Some(info) = config.addons.linked.get(identifier) {
        return Some(info.folder_name.clone());
    }

    // Check globals config
    if let Some(info) = config.addons.globals.get(identifier) {
        return Some(info.folder_name.clone());
    }

    // Derive from the identifier (use the asset slug as folder name)
    identifier.split('/').next_back().map(|s| s.to_string())
}

/// Remove a project from a linked addon's reference list.
/// If no projects remain, remove the entry and clean up the global store.
fn remove_project_reference(config: &mut Config, identifier: &str, project_key: &str) {
    let should_remove = if let Some(info) = config.addons.linked.get_mut(identifier) {
        info.projects.retain(|p| p != project_key);
        info.projects.is_empty()
    } else {
        false
    };
    if should_remove && let Some(info) = config.addons.linked.remove(identifier) {
        storage::cleanup_global_store(identifier, Some(&info.version));
    }
}
