use anyhow::{Context, Result};
use crate::config::Config;
use crate::godot;
use crate::project;

pub fn run(config: &mut Config) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let project_file = cwd.join("project.godot");

    if !project_file.exists() {
        println!("No Godot project found in current directory.");
        println!("Use `gdio --help` for usage information.");
        return Ok(());
    }

    let project_path = cwd.to_string_lossy().to_string();
    let project_name = project::parse_project_name(&project_file)
        .unwrap_or_else(|| "Unknown Project".to_string());
    let godot_version = project::parse_godot_version(&project_file);

    println!("Found project: {} ({})", project_name, project_path);

    // Check if we've opened this project before
    if let Some(existing) = config.projects.get(&project_path)
        && let Some(ref editor_version) = existing.bound_editor
        && let Some(editor) = config.find_editor_for_version(editor_version)
        && editor.path.exists()
    {
        println!("Opening with {}...", editor.name);
        godot::open_project_editor_mode(&editor.path, &project_file)?;

        let now = chrono_now();
        config.register_project(crate::config::ProjectInfo {
            path: cwd,
            name: project_name,
            bound_editor: Some(editor_version.clone()),
            last_opened: Some(now),
        });
        config.save()?;
        return Ok(());
    }

    // Try to find editor for detected version
    if let Some(ref version) = godot_version {
        println!("Project requires Godot {}", version);
        if let Some(editor) = config.find_editor_for_version(version)
            && editor.path.exists()
        {
            println!("Found editor: {}", editor.name);
            godot::open_project_editor_mode(&editor.path, &project_file)?;

            let now = chrono_now();
            config.register_project(crate::config::ProjectInfo {
                path: cwd,
                name: project_name,
                bound_editor: Some(editor.version.clone()),
                last_opened: Some(now),
            });
            config.save()?;
            return Ok(());
        }

        println!("Godot {} editor not found.", version);
        println!("Use `gdio add {}` to install it.", version);
    } else {
        println!("Could not determine Godot version from project.godot");
        println!("Use `gdio --help` for usage information.");
    }

    Ok(())
}

pub(crate) fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{}", secs)
}
