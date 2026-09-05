pub mod clone;
pub mod editor;
pub mod install;
pub mod secure;
pub mod templates;

use anyhow::{Context, Result};
use std::path::Path;

/// Detect Godot version from source directory by reading version.py
pub fn detect_godot_version(godot_dir: &Path) -> Result<String> {
    let version_py = godot_dir.join("version.py");
    if !version_py.exists() {
        anyhow::bail!("version.py not found. Ensure you are in a Godot source directory.");
    }

    let content = std::fs::read_to_string(&version_py).context("Failed to read version.py")?;

    let mut major: Option<u32> = None;
    let mut minor: Option<u32> = None;
    let mut patch: Option<u32> = None;
    let mut status: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("major") {
            let val = val.trim().strip_prefix('=').unwrap_or(val).trim();
            major = val.parse().ok();
        } else if let Some(val) = line.strip_prefix("minor") {
            let val = val.trim().strip_prefix('=').unwrap_or(val).trim();
            minor = val.parse().ok();
        } else if let Some(val) = line.strip_prefix("patch") {
            let val = val.trim().strip_prefix('=').unwrap_or(val).trim();
            patch = val.parse().ok();
        } else if let Some(val) = line.strip_prefix("status") {
            let val = val.trim().strip_prefix('=').unwrap_or(val).trim();
            let val = val.trim_matches('"').trim_matches('\'');
            if !val.is_empty() {
                status = Some(val.to_string());
            }
        }
    }

    let major = major.context("Could not parse 'major' from version.py")?;
    let minor = minor.context("Could not parse 'minor' from version.py")?;
    let patch = patch.unwrap_or(0);
    let status = status.unwrap_or_else(|| "stable".to_string());

    Ok(format!("{}.{}.{}.{}", major, minor, patch, status))
}

/// Merge base args with extra args, allowing extra_args to override built-in keys.
///
/// For any `key=value` in `extra_args`, removes the matching `key=...` from `base_args`
/// before appending all extra_args. This ensures user-supplied values win.
pub fn merge_args(base_args: Vec<String>, extra_args: &[String]) -> Vec<String> {
    let extra_keys: Vec<&str> = extra_args
        .iter()
        .filter_map(|a| a.split_once('=').map(|(k, _)| k))
        .collect();

    let mut result: Vec<String> = base_args
        .into_iter()
        .filter(|a| {
            !a.split_once('=')
                .is_some_and(|(k, _)| extra_keys.contains(&k))
        })
        .collect();

    result.extend(extra_args.iter().cloned());
    result
}

/// Run scons with the given arguments, merging extra_args to allow overrides
pub async fn run_scons(args: &[String], extra_args: &[String]) -> Result<()> {
    use tokio::process::Command;

    let mut args = args.to_vec();
    args.push("verbose=yes".to_string());
    args.push("warnings=all".to_string());
    args.push("cache_dir=.scons_cache".to_string());

    let args = merge_args(args, extra_args);

    println!("[builder] scons {}", args.join(" "));

    let status = Command::new("scons")
        .args(&args)
        .status()
        .await
        .context("Failed to run scons. Is scons installed?")?;

    if !status.success() {
        anyhow::bail!("scons failed with exit code: {:?}", status.code());
    }

    Ok(())
}
