use crate::config::{self, Config};
use crate::godot;
use anyhow::Result;
use console::Style;

pub async fn run(config: &mut Config) -> Result<()> {
    if config.projects.is_empty() {
        println!("No projects registered.");
        println!("Use `gdio` in a project directory to register it.");
        return Ok(());
    }

    super::shared::cleanup_missing_projects(config)?;

    if config.projects.is_empty() {
        println!("No projects registered.");
        println!("Use `gdio` in a project directory to register it.");
        return Ok(());
    }

    let mut projects: Vec<_> = config.projects.values().cloned().collect();
    projects.sort_by(|a, b| b.last_opened.cmp(&a.last_opened));

    println!("{}\n", Style::new().blue().apply_to("Press Ctrl+C to exit"));
    println!("{:<40} {:<20} Last Opened", "Project", "Editor");
    println!("{}", "-".repeat(90));

    let items: Vec<String> = projects
        .iter()
        .map(|p| {
            let editor = p
                .bound_editor
                .as_ref()
                .and_then(|v| config.find_editor_for_version(v))
                .map_or_else(|| "none".to_string(), |e| e.name.clone());
            let time = p
                .last_opened
                .as_ref()
                .map_or_else(|| "never".to_string(), |s| config::format_relative_time(s));
            let name: String = p.name.chars().take(37).collect();
            let name = if name.len() < p.name.len() {
                format!("{}...", name)
            } else {
                name
            };
            let editor_name: String = editor.chars().take(17).collect();
            let editor_name = if editor_name.len() < editor.len() {
                format!("{}...", editor_name)
            } else {
                editor_name
            };
            format!("{:<40} {:<20} {}", name, editor_name, time)
        })
        .collect();

    let selection = dialoguer::FuzzySelect::new()
        .items(&items)
        .default(0)
        .interact()?;

    println!();
    let project = projects[selection].clone();

    let mode_options = &["edit", "game"];
    let mode = dialoguer::Select::new()
        .with_prompt("Open mode")
        .items(mode_options)
        .default(0)
        .interact()?;
    let is_game = mode == 1;

    let project_file = project.path.join("project.godot");
    if !project_file.exists() {
        anyhow::bail!("Project file not found: {}", project_file.display());
    }

    let editor = super::shared::resolve_editor(config, project.bound_editor.as_deref()).await?;

    if is_game {
        println!("Opening {} in game mode...", project.name);
        godot::open_project_game_mode(&editor.path, &project_file)?;
    } else {
        println!("Opening {} in editor...", project.name);
        godot::open_project_editor_mode(&editor.path, &project_file)?;
    }

    super::shared::register_opened_project(config, project.path, project.name, &editor.version)?;
    Ok(())
}
