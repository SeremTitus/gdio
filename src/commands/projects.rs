use anyhow::{Context, Result};
use console::Style;
use crate::config::{self, Config};
use crate::godot;

pub fn run(config: &mut Config) -> Result<()> {
    if config.projects.is_empty() {
        println!("No projects registered.");
        println!("Use `gdio` in a project directory to register it.");
        return Ok(());
    }

    let mut removed_count = 0u32;
    let paths_to_check: Vec<String> = config.projects.keys().cloned().collect();
    for path in paths_to_check {
        let project_file = std::path::Path::new(&path).join("project.godot");
        if !project_file.exists()
            && let Some(p) = config.remove_project(&path)
        {
            println!("Removed missing project: {} ({})", p.name, path);
            removed_count += 1;
        }
    }
    if removed_count > 0 {
        config.save()?;
        println!();
    }

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
                .map(|e| e.name.clone())
                .unwrap_or_else(|| "none".to_string());
            let time = p
                .last_opened
                .as_ref()
                .map(|s| config::format_relative_time(s))
                .unwrap_or_else(|| "never".to_string());
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
        anyhow::bail!(
            "Project file not found: {}",
            project_file.display()
        );
    }

    let mut editor = project
        .bound_editor
        .as_ref()
        .and_then(|v| config.find_editor_for_version(v))
        .cloned();

    if let Some(ref e) = editor
        && !e.path.exists()
    {
        println!("Editor binary not found: {}", e.path.display());
        editor = None;
    }

    let editor = match editor {
        Some(e) => e,
        None => {
            let mut options: Vec<String> = config.editors.values().map(|e| e.name.clone()).collect();
            options.push("[add editor]".to_string());

            let idx = dialoguer::FuzzySelect::new()
                .with_prompt("Select editor")
                .items(&options)
                .default(0)
                .interact()?;

            if options[idx] == "[add editor]" {
                let version: String = dialoguer::Input::new()
                    .with_prompt("Editor version (e.g. 4.7, 4.7-stable, or path)")
                    .interact_text()?;
                let csharp = dialoguer::Confirm::new()
                    .with_prompt("C# support?")
                    .default(false)
                    .interact()?;
                crate::commands::add::run(&version, None, csharp, config)?;
                config.editors.values().last().cloned()
                    .context("Editor was not added")?
            } else {
                let editors: Vec<_> = config.editors.values().cloned().collect();
                editors[idx].clone()
            }
        }
    };

    if is_game {
        println!("Opening {} in game mode...", project.name);
        godot::open_project_game_mode(&editor.path, &project_file)?;
    } else {
        println!("Opening {} in editor...", project.name);
        godot::open_project_editor_mode(&editor.path, &project_file)?;
    }

    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();

    config.register_project(crate::config::ProjectInfo {
        path: project.path.clone(),
        name: project.name.clone(),
        bound_editor: Some(editor.version.clone()),
        last_opened: Some(now),
    });
    config.save()?;
    Ok(())
}
