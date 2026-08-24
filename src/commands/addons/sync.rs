use super::{api, storage};
use crate::config::Config;
use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;

/// Synchronizes linked and global addons into the current project.
///
/// # Step 1: Linked addons
/// Regenerates symlinks for addons tracked in `.gdio`. If a symlink is missing
/// or points to the wrong target, it is recreated.
///
/// # Step 2: Global addons
/// Checks each addon in `config.addons.globals`. If the addon is missing from
/// this project (and not in the exclusion list), it is installed either as
/// a symlink (if also linked) or as a copy.
///
/// # Step 3: Addon-defined dependencies
/// Walks each addon in `addons/` and checks for a `.gdio` file inside it.
/// Any addons declared there are installed into the project. Newly installed
/// addons are checked recursively (with cycle detection).
pub async fn run(config: &mut Config, project_dir: &Path) -> Result<()> {
    let gdio = storage::read_gdio(project_dir);
    let addons_dir = project_dir.join("addons");
    let global_dir = Config::get_global_addons_dir();

    let mut anything_done = false;

    // Step 1: Sync linked addons tracked in .gdio
    for identifier in gdio.addons.keys() {
        let parts: Vec<&str> = identifier.splitn(2, '/').collect();
        if parts.len() != 2 {
            continue;
        }
        let (publisher, asset) = (parts[0], parts[1]);

        let linked_info = config.addons.linked.get(identifier);
        let folder_name = linked_info
            .map(|g| g.folder_name.as_str())
            .unwrap_or_else(|| asset);
        let version = linked_info.map(|g| g.version.as_str()).unwrap_or("unknown");

        let global_addon_dir = global_dir.join(format!("{}_{}_{}", publisher, asset, version));
        let global_addon_content_dir = global_addon_dir.join(folder_name);
        let link_path = addons_dir.join(folder_name);

        let needs_create = if link_path.symlink_metadata().is_ok() {
            if !storage::is_symlink(&link_path) {
                println!("  {}: exists but not a symlink, skipping", identifier);
                continue;
            }
            match storage::symlink_target(&link_path) {
                Ok(target) => target != global_addon_content_dir,
                Err(_) => true,
            }
        } else {
            true
        };

        if needs_create {
            if !global_addon_content_dir.exists() {
                // Global store missing — re-download from repository
                let gdio_entry = gdio.addons.get(identifier);
                let repo_url = gdio_entry
                    .map(|e| e.repository.as_str())
                    .unwrap_or("https://store.godotengine.org");
                let version_str = gdio_entry.map(|e| e.version.as_str()).unwrap_or(version);

                println!("  {}: re-downloading v{}...", identifier, version_str);
                let client = reqwest::Client::builder().user_agent("gdio").build()?;

                let mut download_url = None;
                for repo in &config.addons.repositories {
                    if repo.url != repo_url && !repo_url.is_empty() {
                        continue;
                    }
                    if let Ok(releases) =
                        api::fetch_releases(&client, &repo.url, publisher, asset).await
                        && let Some(r) = releases.iter().find(|r| r.version == version_str)
                    {
                        download_url = Some(r.download_url.clone());
                        break;
                    }
                }

                let download_url = match download_url {
                    Some(u) => u,
                    None => {
                        // Fallback: try any release
                        for repo in &config.addons.repositories {
                            if let Ok(releases) =
                                api::fetch_releases(&client, &repo.url, publisher, asset).await
                                && let Some(r) = releases.first()
                            {
                                download_url = Some(r.download_url.clone());
                                break;
                            }
                        }
                        match download_url {
                            Some(u) => u,
                            None => {
                                println!("  {}: no release found, skipping", identifier);
                                continue;
                            }
                        }
                    }
                };

                let cache_dir = Config::get_addons_cache_dir();
                let zip_name = format!("{}_v{}.zip", identifier.replace('/', "_"), version_str);
                let zip_path =
                    match api::download_zip(&client, &download_url, &cache_dir, &zip_name).await {
                        Ok(p) => p,
                        Err(e) => {
                            println!("    download failed: {}", e);
                            continue;
                        }
                    };

                match storage::extract_addon(&zip_path, &global_addon_dir, true) {
                    Ok(extracted_folder) => {
                        let _ = std::fs::remove_file(&zip_path);
                        let global_addon_content_dir = global_addon_dir.join(&extracted_folder);
                        std::fs::create_dir_all(&addons_dir)?;
                        storage::create_symlink(&global_addon_content_dir, &link_path)?;
                        storage::enable_plugin(project_dir, &extracted_folder)?;
                        println!(
                            "  synced (re-downloaded): {} -> {}",
                            folder_name,
                            global_addon_content_dir.display()
                        );
                        anything_done = true;
                    }
                    Err(e) => {
                        println!("    extraction failed: {}", e);
                        let _ = std::fs::remove_file(&zip_path);
                        continue;
                    }
                }
            } else {
                std::fs::create_dir_all(&addons_dir)?;
                storage::create_symlink(&global_addon_content_dir, &link_path)?;
                storage::enable_plugin(project_dir, folder_name)?;
                println!(
                    "  synced: {} -> {}",
                    folder_name,
                    global_addon_content_dir.display()
                );
                anything_done = true;
            }
        }
    }

    // Step 2: Sync global addons
    let client = reqwest::Client::builder().user_agent("gdio").build()?;
    let project_key = project_dir.to_string_lossy().to_string();
    let godot_version = config
        .projects
        .get(&project_key)
        .and_then(|p| p.bound_editor.clone());

    for (identifier, global_info) in &config.addons.globals {
        if config
            .addons
            .globals_exclusions
            .get(identifier)
            .map(|e| e.contains(&project_key))
            .unwrap_or(false)
        {
            continue;
        }

        let addon_path = addons_dir.join(&global_info.folder_name);
        if addon_path.exists() || addon_path.symlink_metadata().is_ok() {
            continue;
        }

        let parts: Vec<&str> = identifier.splitn(2, '/').collect();
        if parts.len() != 2 {
            println!("  {}: invalid identifier, skipping", identifier);
            continue;
        }
        let (publisher, asset) = (parts[0], parts[1]);

        if global_info.repository != "https://store.godotengine.org"
            && !global_info.repository.is_empty()
        {
            println!(
                "  {}: third-party global addon not cached, skipping",
                global_info.folder_name
            );
            continue;
        }

        let (download_url, version) = if let Some(ref pinned_version) = global_info.version {
            let pinned_version = pinned_version.clone();
            let mut url = None;
            for repo in &config.addons.repositories {
                if let Ok(releases) =
                    api::fetch_releases(&client, &repo.url, publisher, asset).await
                    && let Some(r) = releases.iter().find(|r| r.version == pinned_version)
                {
                    url = Some(r.download_url.clone());
                    break;
                }
            }
            match url {
                Some(u) => (u, pinned_version),
                None => {
                    println!(
                        "  {}: pinned version v{} not found, skipping",
                        identifier, pinned_version
                    );
                    continue;
                }
            }
        } else {
            let mut found: Option<(String, String)> = None;
            for repo in &config.addons.repositories {
                match api::fetch_releases(&client, &repo.url, publisher, asset).await {
                    Ok(releases) => {
                        let compatible = if let Some(ref gv) = godot_version {
                            api::list_compatible_releases(&releases, gv)
                        } else {
                            releases.iter().collect()
                        };
                        if let Some(r) = compatible.first() {
                            found = Some((r.download_url.clone(), r.version.clone()));
                            break;
                        }
                    }
                    Err(e) => eprintln!("    Skipping {}: {}", repo.name, e),
                }
            }
            match found {
                Some(f) => f,
                None => {
                    println!("  {}: no compatible release found, skipping", identifier);
                    continue;
                }
            }
        };

        let addon_global_dir = global_dir.join(format!("{}_{}_{}", publisher, asset, version));
        let global_addon_content_dir = addon_global_dir.join(&global_info.folder_name);

        if global_addon_content_dir.exists() {
            std::fs::create_dir_all(&addons_dir)?;
            if global_info.linked {
                storage::create_symlink(&global_addon_content_dir, &addon_path)?;
                println!(
                    "  installed (global/symlink): {} v{}",
                    global_info.folder_name, version
                );
            } else {
                storage::copy_dir_all(&global_addon_content_dir, &addon_path)?;
                println!(
                    "  installed (global/copied): {} v{}",
                    global_info.folder_name, version
                );
            }
            storage::enable_plugin(project_dir, &global_info.folder_name)?;
            anything_done = true;
        } else {
            println!("  {}: downloading v{} from store...", identifier, version);
            let cache_dir = Config::get_addons_cache_dir();
            let zip_name = format!("{}_v{}.zip", identifier.replace('/', "_"), version);
            let zip_path =
                match api::download_zip(&client, &download_url, &cache_dir, &zip_name).await {
                    Ok(p) => p,
                    Err(e) => {
                        println!("    download failed: {}", e);
                        continue;
                    }
                };

            let folder_name = match storage::extract_addon(&zip_path, &addon_global_dir, true) {
                Ok(name) => name,
                Err(e) => {
                    println!("    extraction failed: {}", e);
                    let _ = std::fs::remove_file(&zip_path);
                    continue;
                }
            };
            let _ = std::fs::remove_file(&zip_path);

            let global_addon_content_dir = addon_global_dir.join(&folder_name);

            std::fs::create_dir_all(&addons_dir)?;
            if global_info.linked {
                storage::create_symlink(&global_addon_content_dir, &addon_path)?;
                println!("  installed (global/symlink): {} v{}", folder_name, version);
            } else {
                storage::copy_dir_all(&global_addon_content_dir, &addon_path)?;
                println!("  installed (global/copied): {} v{}", folder_name, version);
            }
            storage::enable_plugin(project_dir, &folder_name)?;

            anything_done = true;
        }
    }

    // Step 3: Resolve addon-defined dependencies (recursive with cycle detection)
    let mut visited = HashSet::new();
    let step3_done = resolve_addon_dependencies(config, project_dir, &mut visited).await?;
    anything_done = anything_done || step3_done;

    if !anything_done {
        println!("  All addons are up to date.");
    }

    Ok(())
}

/// Auto-sync entry point called from `gdio` (default command).
///
/// Only runs if the project has a `.gdio` file.
pub async fn run_sync(config: &mut Config, project_dir: &std::path::Path) -> Result<()> {
    // Check if this project has a .gdio file
    let gdio_file = project_dir.join(".gdio");
    if !gdio_file.exists() {
        return Ok(());
    }

    // Run the full sync
    println!("Syncing addons...");
    run(config, project_dir).await?;

    // Increment sync counter and run periodic cleanup
    config.addons.sync_count += 1;
    if config.addons.sync_count >= 20 {
        config.addons.sync_count = 0;
        cleanup_orphaned_links(config);
    }
    config.save()?;

    Ok(())
}

/// Walk each addon in `{project_dir}/addons/` and check for a `.gdio` file.
///
/// Any addons declared in an addon's `.gdio` are installed into the project.
/// Newly installed addons are checked recursively. `visited` tracks identifiers
/// already processed to prevent cycles.
fn resolve_addon_dependencies<'a>(
    config: &'a mut Config,
    project_dir: &'a Path,
    visited: &'a mut HashSet<String>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool>> + Send + 'a>> {
    Box::pin(async move {
        let addons_dir = project_dir.join("addons");
        if !addons_dir.exists() {
            return Ok(false);
        }
        let global_dir = Config::get_global_addons_dir();

        let mut anything_done = false;

        // Collect addon dirs first to avoid borrow issues during iteration
        let addon_dirs: Vec<_> = std::fs::read_dir(&addons_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().ok().is_some_and(|t| t.is_dir()))
            .collect();

        for entry in &addon_dirs {
            let addon_path = entry.path();
            let addon_gdio = storage::read_gdio(&addon_path);

            if addon_gdio.addons.is_empty() {
                continue;
            }

            // Check for cycles
            let addon_name = entry.file_name().to_string_lossy().to_string();
            if !visited.insert(addon_name.clone()) {
                continue;
            }

            for identifier in addon_gdio.addons.keys() {
                let parts: Vec<&str> = identifier.splitn(2, '/').collect();
                if parts.len() != 2 {
                    println!("  dep {}: invalid identifier, skipping", identifier);
                    continue;
                }
                let (publisher, asset) = (parts[0], parts[1]);

                let folder_name = config
                    .addons
                    .linked
                    .get(identifier)
                    .map(|g| g.folder_name.clone())
                    .unwrap_or_else(|| asset.to_string());

                let addon_path_in_project = addons_dir.join(&folder_name);

                // Skip only if both the project's .gdio tracks it AND the directory exists
                let project_gdio = storage::read_gdio(project_dir);
                let in_project_gdio = project_gdio.addons.contains_key(identifier);
                let dir_exists = addon_path_in_project.exists()
                    || addon_path_in_project.symlink_metadata().is_ok();
                if in_project_gdio && dir_exists {
                    continue;
                }

                // Try to find it in the global store (from a previous install or linked addon)
                let linked_info = config.addons.linked.get(identifier);
                let version = linked_info.map(|g| g.version.as_str()).unwrap_or("unknown");

                let global_addon_dir =
                    global_dir.join(format!("{}_{}_{}", publisher, asset, version));
                let global_addon_content_dir = global_addon_dir.join(&folder_name);

                if global_addon_content_dir.exists() {
                    // Found in global store — install
                    std::fs::create_dir_all(&addons_dir)?;
                    storage::create_symlink(&global_addon_content_dir, &addon_path_in_project)?;
                    storage::enable_plugin(project_dir, &folder_name)?;
                    println!(
                        "  dep installed (symlink): {} -> {}",
                        folder_name,
                        global_addon_content_dir.display()
                    );
                    anything_done = true;

                    // Recurse into the newly installed addon
                    let new_done = resolve_addon_dependencies(config, project_dir, visited).await?;
                    anything_done = anything_done || new_done;
                } else {
                    // Not in global store — try to download from repository
                    println!("  dep {}: resolving...", identifier);
                    let client = reqwest::Client::builder().user_agent("gdio").build()?;

                    let mut found: Option<(String, String, String)> = None; // (download_url, version, repo_url)
                    for repo in &config.addons.repositories {
                        match api::fetch_releases(&client, &repo.url, publisher, asset).await {
                            Ok(releases) => {
                                if let Some(r) = releases.first() {
                                    found = Some((
                                        r.download_url.clone(),
                                        r.version.clone(),
                                        repo.url.clone(),
                                    ));
                                    break;
                                }
                            }
                            Err(e) => {
                                println!("    Skipping {}: {}", repo.name, e);
                            }
                        }
                    }

                    let (download_url, dep_version, repo_url) = match found {
                        Some(f) => f,
                        None => {
                            println!("  dep {}: no release found, skipping", identifier);
                            continue;
                        }
                    };

                    // Download and extract
                    let cache_dir = Config::get_addons_cache_dir();
                    let zip_name = format!("{}_v{}.zip", identifier.replace('/', "_"), dep_version);
                    let zip_path = match api::download_zip(
                        &client,
                        &download_url,
                        &cache_dir,
                        &zip_name,
                    )
                    .await
                    {
                        Ok(p) => p,
                        Err(e) => {
                            println!("    dep download failed: {}", e);
                            continue;
                        }
                    };

                    let extracted_folder =
                        match storage::extract_addon(&zip_path, &global_addon_dir, true) {
                            Ok(name) => name,
                            Err(e) => {
                                println!("    dep extraction failed: {}", e);
                                let _ = std::fs::remove_file(&zip_path);
                                continue;
                            }
                        };
                    let _ = std::fs::remove_file(&zip_path);

                    let global_addon_content_dir = global_addon_dir.join(&extracted_folder);

                    // Symlink into project
                    std::fs::create_dir_all(&addons_dir)?;
                    storage::create_symlink(&global_addon_content_dir, &addon_path_in_project)?;
                    storage::enable_plugin(project_dir, &extracted_folder)?;

                    // Update .gdio in project
                    storage::add_linked(
                        project_dir,
                        identifier,
                        &dep_version,
                        &repo_url,
                        &extracted_folder,
                    )?;

                    println!(
                        "  dep installed: {} v{} -> {}",
                        identifier, dep_version, extracted_folder
                    );

                    // Track in linked config
                    use crate::config::LinkedAddonInfo;
                    let project_key = project_dir.to_string_lossy().to_string();
                    let entry = config
                        .addons
                        .linked
                        .entry(identifier.to_string())
                        .or_insert_with(|| LinkedAddonInfo {
                            version: dep_version.clone(),
                            folder_name: extracted_folder.clone(),
                            projects: Vec::new(),
                        });
                    if !entry.projects.contains(&project_key) {
                        entry.projects.push(project_key);
                    }
                    entry.version = dep_version;
                    entry.folder_name = extracted_folder;

                    anything_done = true;

                    // Recurse into the newly installed addon
                    let new_done = resolve_addon_dependencies(config, project_dir, visited).await?;
                    anything_done = anything_done || new_done;
                }
            }
        }

        Ok(anything_done)
    })
}

/// Validate all linked addon references. Remove stale entries and clean up
/// addons with no remaining project references.
fn cleanup_orphaned_links(config: &mut Config) {
    let mut to_remove: Vec<String> = Vec::new();
    let mut to_update: Vec<(String, Vec<String>)> = Vec::new();

    for (identifier, info) in &config.addons.linked {
        let mut valid_projects: Vec<String> = Vec::new();

        for project_path in &info.projects {
            let project_dir = std::path::Path::new(project_path);

            // Check project directory still exists
            if !project_dir.exists() {
                continue;
            }

            // Check the addon is still in the project's .gdio file
            let gdio = crate::commands::addons::storage::read_gdio(project_dir);
            if gdio.addons.contains_key(identifier) {
                valid_projects.push(project_path.clone());
            }
        }

        if valid_projects.is_empty() {
            to_remove.push(identifier.clone());
        } else if valid_projects.len() != info.projects.len() {
            to_update.push((identifier.clone(), valid_projects));
        }
    }

    for (identifier, valid_projects) in to_update {
        if let Some(entry) = config.addons.linked.get_mut(&identifier) {
            entry.projects = valid_projects;
        }
    }

    for identifier in to_remove {
        if let Some(info) = config.addons.linked.remove(&identifier) {
            storage::cleanup_global_store(&identifier, Some(&info.version));
        }
    }
}
