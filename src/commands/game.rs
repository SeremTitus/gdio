use crate::config::Config;
use anyhow::Result;

pub fn run(config: &Config) -> Result<()> {
    let ctx = super::shared::ProjectContext::detect("Unknown Project")?;

    // Find bound editor
    if let Some(editor) = ctx.bound_editor(config) {
        println!("Opening {} in game mode...", ctx.project_name);
        crate::godot::open_project_game_mode(&editor.path, &ctx.project_file)?;
        return Ok(());
    }

    // Try to detect version and find editor
    if let Some((_, editor)) = ctx.find_editor_for_detected_version(config) {
        println!("Opening {} in game mode...", ctx.project_name);
        crate::godot::open_project_game_mode(&editor.path, &ctx.project_file)?;
        return Ok(());
    }

    println!("No editor found for this project.");
    println!("Use `gdio add` to install an editor, then `gdio bind` to bind it.");
    Ok(())
}
