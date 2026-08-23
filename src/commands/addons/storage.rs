use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::config::{GdioAddonEntry, GdioProject};

/// Extract an addon ZIP archive into a destination directory.
///
/// Handles top-level directory detection matching the Godot editor's behavior:
/// - If the ZIP has a single root directory (e.g. "my-addon-main/"), it is stripped.
/// - If the root directory is literally "addons/", it is NOT stripped (preserves addons/X/ structure).
///
/// When `strip_addons_prefix` is true, any leading `addons/` in paths is also stripped
/// (useful for global installs where the store dir shouldn't have an addons/ subdirectory).
///
/// Returns the name of the addon folder (e.g. "gut" or "ruzta").
pub fn extract_addon(zip_path: &Path, dest_dir: &Path, strip_addons_prefix: bool) -> Result<String> {
    // Open and read all entry names from the ZIP archive
    let file = std::fs::File::open(zip_path).context("Failed to open ZIP file")?;
    let mut archive = zip::ZipArchive::new(file).context("Failed to read ZIP archive")?;

    let mut entries = HashSet::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        // Normalize to forward slashes (ZIP crate uses \ on Windows)
        let name = entry.mangled_name().to_string_lossy().replace('\\', "/");
        entries.insert(name);
    }

    // Detect if there's a single top-level directory to strip
    let toplevel_prefix = detect_toplevel_prefix(&entries);

    let mut addon_folder_name = String::new();

    // Extract each entry, stripping the top-level prefix if detected
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        // Normalize to forward slashes (ZIP crate uses \ on Windows)
        let raw_name = entry.mangled_name().to_string_lossy().replace('\\', "/");

        // Compute the relative path after stripping the top-level prefix
        let rel_path = if let Some(ref prefix) = toplevel_prefix {
            // Skip the root directory entry itself (with or without trailing slash)
            let prefix_base = prefix.trim_end_matches('/');
            if raw_name == *prefix || raw_name == prefix_base || raw_name == format!("{}/", prefix_base) {
                continue;
            }
            raw_name.strip_prefix(prefix).unwrap_or(&raw_name)
        } else {
            &raw_name
        };

        // Skip empty paths (can happen with directory entries)
        if rel_path.is_empty() {
            continue;
        }

        // Strip leading "addons/" prefix when requested (for global store installs)
        let final_path = if strip_addons_prefix {
            match rel_path.strip_prefix("addons/") {
                Some(stripped) => stripped,
                None => {
                    // Skip entries that are just the "addons" directory itself
                    if rel_path == "addons" || rel_path == "addons/" {
                        continue;
                    }
                    rel_path
                }
            }
        } else {
            rel_path
        };

        if final_path.is_empty() {
            continue;
        }

        let outpath = dest_dir.join(final_path);

        if entry.is_dir() {
            // Create the directory and record the addon folder name from the first directory entry
            std::fs::create_dir_all(&outpath)?;
            if addon_folder_name.is_empty() {
                let folder = final_path.trim_end_matches('/');
                // For local installs (strip_addons_prefix=false), paths are addons/X/...
                // Skip past the addons/ prefix to get the actual addon folder name
                let name_component = if !strip_addons_prefix {
                    // Strip "addons/" or "addons" prefix
                    if let Some(rest) = folder.strip_prefix("addons/") {
                        rest
                    } else if let Some(rest) = folder.strip_prefix("addons") {
                        rest
                    } else {
                        folder
                    }
                } else {
                    folder
                };
                if let Some(first) = name_component.split('/').next()
                    && !first.is_empty() {
                        addon_folder_name = first.to_string();
                    }
            }
        } else {
            // Ensure parent directories exist, then extract the file
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut entry, &mut outfile)?;
        }
    }

    // Error if we still couldn't determine the addon folder name
    if addon_folder_name.is_empty() {
        anyhow::bail!("Could not determine addon folder name from ZIP");
    }

    Ok(addon_folder_name)
}

/// Detect if all entries in a ZIP share a single top-level directory.
///
/// Returns `Some("prefix/")` if so, `None` if the ZIP has mixed top-level entries
/// or the prefix is literally "addons/" (which should never be stripped).
fn detect_toplevel_prefix(entries: &HashSet<String>) -> Option<String> {
    if entries.is_empty() {
        return None;
    }

    // Find the first component of each entry
    let first_components: Vec<&str> = entries
        .iter()
        .filter_map(|e| {
            let name = e.strip_suffix('/').unwrap_or(e);
            name.split('/').next()
        })
        .filter(|s| !s.is_empty())
        .collect();

    if first_components.is_empty() {
        return None;
    }

    // All entries must share the same first component
    let candidate = first_components[0];
    if !first_components.iter().all(|c| *c == candidate) {
        return None;
    }

    // The candidate must be a directory (i.e., there are deeper paths)
    let has_children = entries.iter().any(|e| {
        let name = e.strip_suffix('/').unwrap_or(e);
        name.starts_with(&format!("{}/", candidate)) && name != candidate
    });
    if !has_children {
        return None;
    }

    // Don't strip "addons/" — preserve the addons/X/ structure
    if candidate == "addons" {
        return None;
    }

    Some(format!("{}/", candidate))
}

/// Create a directory symlink from `link` to `target`.
///
/// On Windows, creates an NTFS junction via `cmd.exe /c mklink /J`
/// (no privileges required).
/// On Unix, uses `symlink` (file or directory symlink).
/// Removes any existing entry at `link` first.
#[cfg(target_os = "windows")]
pub fn create_symlink(target: &Path, link: &Path) -> Result<()> {
    if link.exists() || link.symlink_metadata().is_ok() {
        std::fs::remove_dir_all(link).ok();
    }
    create_junction(target, link)
}

/// Create an NTFS junction point from `link` to `target` using `mklink /J`.
///
/// Junctions work like symlinks for directories but don't require elevated
/// privileges. This is the standard Windows approach for developer tools.
#[cfg(target_os = "windows")]
fn create_junction(target: &Path, link: &Path) -> Result<()> {
    use std::process::Command;

    let target_abs = std::fs::canonicalize(target)
        .context("Failed to resolve absolute target path for junction")?;

    let output = Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(link)
        .arg(&target_abs)
        .output()
        .context("Failed to run mklink")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to create junction: {}", stderr.trim());
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn create_symlink(target: &Path, link: &Path) -> Result<()> {
    if link.exists() || link.symlink_metadata().is_ok() {
        std::fs::remove_dir_all(link).ok();
    }
    std::os::unix::fs::symlink(target, link).context("Failed to create symlink")
}

/// Remove a symlink without deleting its target.
///
/// On Windows, uses `remove_dir` (directory symlinks).
/// On Unix, uses `remove_file` (both file and directory symlinks).
/// Safety check: refuses to remove a non-symlink directory on Windows.
pub fn remove_symlink(link: &Path) -> Result<()> {
    if link.symlink_metadata().is_ok() {
        #[cfg(target_os = "windows")]
        {
            // Safety: don't accidentally remove a real directory
            if link.is_dir() && !is_symlink(link) {
                anyhow::bail!("Refusing to remove non-symlink directory: {}", link.display());
            }
            std::fs::remove_dir(link).context("Failed to remove symlink")?;
        }
        #[cfg(not(target_os = "windows"))]
        {
            std::fs::remove_file(link).context("Failed to remove symlink")?;
        }
    }
    Ok(())
}

/// Check if a path is a symlink (not a regular file or directory).
pub fn is_symlink(path: &Path) -> bool {
    path.symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

/// Read the target path that a symlink points to.
pub fn symlink_target(link: &Path) -> Result<PathBuf> {
    let target = std::fs::read_link(link).context("Failed to read symlink target")?;
    Ok(target)
}

/// Remove a versioned addon directory from the global store.
///
/// Deletes `~/.config/gdio/addons/{publisher}_{asset}_{version}/` if it exists.
/// If `version` is `None`, skips cleanup (can't determine which directory to remove).
pub fn cleanup_global_store(identifier: &str, version: Option<&str>) {
    let version = match version {
        Some(v) => v,
        None => return,
    };
    let parts: Vec<&str> = identifier.splitn(2, '/').collect();
    if parts.len() != 2 {
        return;
    }
    let global_dir = crate::config::Config::get_global_addons_dir();
    let dir_name = format!("{}_{}_{}", parts[0], parts[1], version);
    let path = global_dir.join(&dir_name);
    if path.exists() {
        let _ = std::fs::remove_dir_all(&path);
    }
}

/// Enable a plugin in `project.godot` by adding it to `[editor_plugins].enabled`.
///
/// If the section or key doesn't exist, it is created.
/// If the plugin is already enabled, this is a no-op.
/// Preserves existing line endings and all other content.
pub fn enable_plugin(project_dir: &Path, folder_name: &str) -> Result<()> {
    let project_file = project_dir.join("project.godot");
    if !project_file.exists() {
        return Ok(());
    }

    let plugin_cfg = project_dir.join("addons").join(folder_name).join("plugin.cfg");
    if !plugin_cfg.exists() {
        return Ok(());
    }

    let plugin_path = format!("res://addons/{}/plugin.cfg", folder_name);

    let content = std::fs::read_to_string(&project_file)?;

    // Check if already enabled
    if content.contains(&plugin_path) {
        return Ok(());
    }

    // Detect line ending style and normalize to \n for splitting
    let has_crlf = content.contains("\r\n");
    let normalized = if has_crlf { content.replace("\r\n", "\n") } else { content };

    let mut lines: Vec<String> = normalized.split('\n').map(|s| s.to_string()).collect();

    // Find [editor_plugins] section
    if let Some(section_idx) = lines.iter().position(|l| l.trim() == "[editor_plugins]") {
        // Find enabled= line within this section
        let mut enabled_idx = None;
        for (i, line) in lines.iter().enumerate().skip(section_idx + 1) {
            if line.trim().starts_with('[') {
                break; // next section
            }
            if line.trim().starts_with("enabled=") {
                enabled_idx = Some(i);
                break;
            }
        }

        if let Some(idx) = enabled_idx {
            // Append to existing PackedStringArray
            // Find the ')' that closes PackedStringArray, skipping trailing comments
            let line = &lines[idx];
            let mut close_pos = None;
            let mut search_from = line.len();
            while let Some(pos) = line[..search_from].rfind(')') {
                let after = &line[pos + 1..];
                let trimmed_after = after.trim_start();
                if trimmed_after.is_empty() || trimmed_after.starts_with('#') {
                    close_pos = Some(pos);
                    break;
                }
                search_from = pos;
            }

            if let Some(pos) = close_pos {
                let prefix = &line[..pos];
                if prefix.contains("PackedStringArray(") && prefix.ends_with('"') {
                    let inner = &prefix["enabled=PackedStringArray(".len()..];
                    lines[idx] = format!("enabled=PackedStringArray({},\"{}\")", inner, plugin_path);
                } else {
                    lines[idx] = format!("enabled=PackedStringArray(\"{}\")", plugin_path);
                }
            } else {
                lines[idx] = format!("enabled=PackedStringArray(\"{}\")", plugin_path);
            }
        } else {
            // No enabled= line — insert after section header
            lines.insert(section_idx + 1, format!("enabled=PackedStringArray(\"{}\")", plugin_path));
        }
    } else {
        // No [editor_plugins] section — append at end
        if let Some(last) = lines.last()
            && !last.is_empty()
        {
            lines.push(String::new());
        }
        lines.push("[editor_plugins]".to_string());
        lines.push(format!("enabled=PackedStringArray(\"{}\")", plugin_path));
    }

    let line_sep = if has_crlf { "\r\n" } else { "\n" };
    let new_content = lines.join(line_sep);
    std::fs::write(&project_file, new_content)?;
    Ok(())
}

/// Recursively copy a directory and all its contents to a new location.
pub fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

/// Find a local addon folder in `addons/` by matching the asset slug.
///
/// First checks `{addons}/{asset}` directly, then scans for directories
/// whose `plugin.cfg` `name` field matches the asset slug.
pub fn find_local_addon_folder(project_dir: &Path, asset: &str) -> Option<String> {
    let addons_dir = project_dir.join("addons");
    if !addons_dir.exists() {
        return None;
    }

    // Quick check: does addons/{asset} exist as a directory?
    let direct = addons_dir.join(asset);
    if direct.is_dir() {
        return Some(asset.to_string());
    }

    // Scan for plugin.cfg name matching the asset slug
    for entry in std::fs::read_dir(&addons_dir).ok()? {
        let entry = entry.ok()?;
        if !entry.file_type().ok()?.is_dir() {
            continue;
        }
        let plugin_cfg = entry.path().join("plugin.cfg");
        if !plugin_cfg.exists() {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&plugin_cfg) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("name=") {
                    let name = trimmed.split_once('=').map(|(_, v)| v.trim_matches('"')).unwrap_or("");
                    if name.eq_ignore_ascii_case(asset) {
                        return Some(entry.file_name().to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    None
}

/// Read the `.gdio` project file from a project directory.
///
/// Returns a default (empty) `GdioProject` if the file doesn't exist or can't be parsed.
///
/// The `.gdio` file uses unquoted `[publisher/asset]` section headers for readability,
/// but TOML requires quoting keys containing `/`. This function adds quotes to bare
/// section headers before parsing.
pub fn read_gdio(project_dir: &Path) -> GdioProject {
    let gdio_path = project_dir.join(".gdio");
    if !gdio_path.exists() {
        return GdioProject::default();
    }
    let content = match std::fs::read_to_string(&gdio_path) {
        Ok(c) => c,
        Err(_) => return GdioProject::default(),
    };
    let content: String = content
        .lines()
        .map(quote_bare_toml_key)
        .collect::<Vec<_>>()
        .join("\n");
    toml::from_str(&content).unwrap_or_default()
}

/// Add quotes to a bare TOML section header containing `/`.
///
/// Transforms `[publisher/asset]` into `["publisher/asset"]` so the TOML parser
/// can handle it. Already-quoted headers (`["publisher/asset"]`) and other table
/// headers are left unchanged.
fn quote_bare_toml_key(line: &str) -> String {
    let trimmed = line.trim_start();
    // Match: [ <non-quote chars containing /> ]
    if trimmed.starts_with('[')
        && trimmed.ends_with(']')
        && !trimmed.starts_with("[\"")
        && !trimmed.starts_with("['")
    {
        let inner = &trimmed[1..trimmed.len() - 1];
        if inner.contains('/') && !inner.contains('"') && !inner.contains('#') {
            let indent = &line[..line.len() - trimmed.len()];
            return format!("{}[\"{}\"]", indent, inner);
        }
    }
    line.to_string()
}

/// Write the `.gdio` project file to a project directory.
///
/// Deletes the file if the project has no tracked addons.
///
/// Strips quotes from TOML section headers so the file uses the readable
/// `[publisher/asset]` format instead of `["publisher/asset"]`.
fn write_gdio(project_dir: &Path, gdio: &GdioProject) -> Result<()> {
    let gdio_path = project_dir.join(".gdio");
    if gdio.addons.is_empty() {
        let _ = std::fs::remove_file(&gdio_path);
        return Ok(());
    }
    let content = toml::to_string_pretty(gdio).context("Failed to serialize .gdio file")?;
    let content: String = content
        .lines()
        .map(unquote_toml_key)
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&gdio_path, content)?;
    Ok(())
}

/// Strip quotes from a TOML section header containing `/`.
///
/// Transforms `["publisher/asset"]` into `[publisher/asset]` for readability.
fn unquote_toml_key(line: &str) -> String {
    let trimmed = line.trim_start();
    if trimmed.starts_with("[\"") && trimmed.ends_with("\"]") {
        let inner = &trimmed[2..trimmed.len() - 2];
        // Only unquote keys containing `/` (our addon identifiers)
        if inner.contains('/') && !inner.contains('#') {
            let indent = &line[..line.len() - trimmed.len()];
            return format!("{}[{}]", indent, inner);
        }
    }
    line.to_string()
}

/// Append a linked addon folder to the project's `.gitignore`.
///
/// Adds `addons/{folder_name}/` if not already present.
fn gitignore_add(project_dir: &Path, folder_name: &str) -> Result<()> {
    let gitignore_path = project_dir.join(".gitignore");
    let entry = format!("addons/{}/", folder_name);

    let content = if gitignore_path.exists() {
        std::fs::read_to_string(&gitignore_path)?
    } else {
        String::new()
    };

    if content.lines().any(|l| l.trim() == entry) {
        return Ok(());
    }

    let new_content = if content.is_empty() {
        format!("{}\n", entry)
    } else {
        let suffix = if content.ends_with('\n') { "" } else { "\n" };
        format!("{}{}{}\n", content, suffix, entry)
    };

    std::fs::write(&gitignore_path, new_content)?;
    Ok(())
}

/// Remove an addon entry from the project's `.gitignore`.
fn gitignore_remove(project_dir: &Path, folder_name: &str) -> Result<()> {
    let gitignore_path = project_dir.join(".gitignore");
    if !gitignore_path.exists() {
        return Ok(());
    }

    let entry = format!("addons/{}/", folder_name);
    let content = std::fs::read_to_string(&gitignore_path)?;
    let new_content: String = content
        .lines()
        .filter(|l| l.trim() != entry)
        .collect::<Vec<_>>()
        .join("\n");

    if new_content.len() != content.len() {
        let final_content = if new_content.is_empty() {
            String::new()
        } else if new_content.ends_with('\n') {
            new_content
        } else {
            format!("{}\n", new_content)
        };
        std::fs::write(&gitignore_path, final_content)?;
    }
    Ok(())
}

/// Add a linked addon to the `.gdio` file and `.gitignore`.
///
/// Updates `.gdio` with the identifier/version/repository, and ensures
/// `addons/{folder_name}/` is in `.gitignore`.
pub fn add_linked(
    project_dir: &Path,
    identifier: &str,
    version: &str,
    repository: &str,
    folder_name: &str,
) -> Result<()> {
    let mut gdio = read_gdio(project_dir);
    gdio.addons.insert(
        identifier.to_string(),
        GdioAddonEntry {
            version: version.to_string(),
            repository: repository.to_string(),
        },
    );
    write_gdio(project_dir, &gdio)?;
    gitignore_add(project_dir, folder_name)?;
    Ok(())
}

/// Remove a linked addon from the `.gdio` file and `.gitignore`.
///
/// Removes the identifier from `.gdio` (if present) and `addons/{folder_name}/` from `.gitignore`.
pub fn remove_linked(project_dir: &Path, identifier: &str, folder_name: &str) -> Result<()> {
    let mut gdio = read_gdio(project_dir);
    gdio.addons.remove(identifier);
    write_gdio(project_dir, &gdio)?;
    gitignore_remove(project_dir, folder_name)?;
    Ok(())
}
