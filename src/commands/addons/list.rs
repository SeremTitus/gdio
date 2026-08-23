use super::storage;
use crate::config::Config;
use anyhow::{Context, Result};

/// List addons in the current project or globally linked addons.
///
/// - No flags: list installed addons in `addons/` directory
/// - `--linked`: list globally linked addons from config
pub fn run(config: &Config, linked: bool) -> Result<()> {
    if linked {
        run_list_linked(config)
    } else {
        run_list_local()
    }
}

/// List addons installed in the current project's `addons/` directory.
fn run_list_local() -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let addons_dir = cwd.join("addons");

    if !addons_dir.exists() {
        println!("No addons directory found.");
        return Ok(());
    }

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

    println!("Installed addons:");
    for entry in &entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let is_link = storage::is_symlink(&entry.path());
        let tag = if is_link { " (global)" } else { "" };
        println!("  {}{}", name, tag);
    }

    Ok(())
}

/// List globally linked addons registered in config.
fn run_list_linked(config: &Config) -> Result<()> {
    if config.addons.linked.is_empty() {
        println!("No linked addons registered.");
        return Ok(());
    }

    println!("Linked addons:");
    for (ident, info) in &config.addons.linked {
        println!(
            "  {: <30} v{:<10} {} ({} projects)",
            ident,
            info.version,
            info.folder_name,
            info.projects.len()
        );
    }

    Ok(())
}
