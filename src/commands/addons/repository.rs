use crate::config::{Config, Repository};
use anyhow::Result;

/// - No arguments: list all registered repositories
/// - With URL: toggle — if the URL is already registered, remove it; otherwise add it
///
/// Repository names are derived from the URL hostname.
/// The default Godot Asset Store is hardcoded as `godot-official-store`.
pub fn run(config: &mut Config, url: Option<&str>) -> Result<()> {
    match url {
        None => run_list(config),
        Some(url) => run_toggle(config, url),
    }
}

/// Toggle a repository by URL.
///
/// If the URL is already registered, remove it (unless it's the default).
/// If not registered, add it with a name derived from the hostname.
fn run_toggle(config: &mut Config, url: &str) -> Result<()> {
    // Check if this URL is already registered
    let existing_idx = config.addons.repositories.iter().position(|r| r.url == url);

    if let Some(idx) = existing_idx {
        // Already registered — remove it (unless it's the default)
        let repo = &config.addons.repositories[idx];
        if repo.name == "godot-official-store" {
            anyhow::bail!("Cannot remove the default Godot Asset Store repository.");
        }
        let removed = config.addons.repositories.remove(idx);
        config.save()?;
        println!("Removed repository '{}' ({})", removed.name, removed.url);
    } else {
        // Not registered — add it
        let name = derive_name(url);
        config.addons.repositories.push(Repository {
            name: name.clone(),
            url: url.to_string(),
        });
        config.save()?;
        println!("Added repository '{}' ({})", name, url);
    }

    Ok(())
}

/// List all registered addon repositories.
fn run_list(config: &Config) -> Result<()> {
    if config.addons.repositories.is_empty() {
        println!("No repositories registered.");
        return Ok(());
    }

    println!("Repositories:");
    for repo in &config.addons.repositories {
        let default = if repo.name == "godot-official-store" {
            " (default)"
        } else {
            ""
        };
        println!("  {: <20} {}{}", repo.name, repo.url, default);
    }

    Ok(())
}

fn derive_name(url: &str) -> String {
    let without_proto = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");

    let host_part = without_proto.split('/').next().unwrap_or("unknown");

    if host_part == "store.godotengine.org" {
        return "godot-official-store".to_string();
    }

    // Strip www. prefix
    let host_part = host_part.strip_prefix("www.").unwrap_or(host_part);

    // Include port in name if present (e.g. "host:8080")
    let mut name = host_part.to_string();

    // Append first non-empty path segment to disambiguate same-host repos
    let path_segments: Vec<&str> = without_proto
        .split('/')
        .skip(1) // skip host:port
        .filter(|s| !s.is_empty())
        .collect();
    if let Some(segment) = path_segments.first() {
        name.push('_');
        name.push_str(segment);
    }

    name
}
