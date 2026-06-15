use anyhow::{Context, Result};
use crate::config::Config;
use crate::project;

pub fn run(config: &mut Config) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let project_file = cwd.join("project.godot");

    if !project_file.exists() {
        anyhow::bail!("No Godot project found in current directory.");
    }

    let project_path = cwd.to_string_lossy().to_string();
    let project_name = project::parse_project_name(&project_file)
        .unwrap_or_else(|| "Unknown Project".to_string());

    // Find bound editor
    if let Some(existing) = config.projects.get(&project_path)
        && let Some(ref editor_version) = existing.bound_editor
        && let Some(editor) = config.find_editor_for_version(editor_version)
    {
        println!("Opening {} in game mode...", project_name);
        crate::godot::open_project_game_mode(&editor.path, &project_file)?;
        return Ok(());
    }

    // Try to detect version and find editor
    if let Some(version) = project::parse_godot_version(&project_file)
        && let Some(editor) = config.find_editor_for_version(&version)
    {
        println!("Opening {} in game mode...", project_name);
        crate::godot::open_project_game_mode(&editor.path, &project_file)?;
        return Ok(());
    }

    println!("No editor found for this project.");
    println!("Use `gdio add` to install an editor, then `gdio update` to bind it.");
    Ok(())
}