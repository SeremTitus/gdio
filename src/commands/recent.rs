use anyhow::{Context, Result};
use crate::config::Config;
use crate::godot;

pub fn run(config: &mut Config) -> Result<()> {
    if config.projects.is_empty() {
        println!("No projects registered.");
        println!("Use `gdio` in a project directory to register it.");
        return Ok(());
    }

    let mut projects: Vec<_> = config.projects.values().cloned().collect();
    projects.sort_by(|a, b| b.last_opened.cmp(&a.last_opened));

    let mut removed_any = false;
    let mut recent = None;
    for project in &projects {
        let project_file = project.path.join("project.godot");
        if !project_file.exists() {
            println!("Removed missing project: {} ({})", project.name, project.path.display());
            config.remove_project(&project.path.to_string_lossy());
            removed_any = true;
        } else {
            recent = Some(project.clone());
            break;
        }
    }

    if removed_any {
        config.save()?;
        println!();
    }

    let recent = match recent {
        Some(p) => p,
        None => {
            println!("No recent project found.");
            return Ok(());
        }
    };

    println!(
        "Opening recent project: {} ({})",
        recent.name,
        recent.path.display()
    );

    let project_file = recent.path.join("project.godot");
    if !project_file.exists() {
        anyhow::bail!("Project file not found: {}", project_file.display());
    }

    let mut editor = recent
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

    godot::open_project_editor_mode(&editor.path, &project_file)?;

    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();

    config.register_project(crate::config::ProjectInfo {
        path: recent.path.clone(),
        name: recent.name.clone(),
        bound_editor: Some(editor.version.clone()),
        last_opened: Some(now),
    });
    config.save()?;
    Ok(())
}