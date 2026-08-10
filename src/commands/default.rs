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
        let project_info = crate::config::ProjectInfo {
            path: cwd,
            name: project_name,
            bound_editor: Some(editor_version.clone()),
            last_opened: Some(now),
        };
        config.register_project(&project_info);
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
            let project_info = crate::config::ProjectInfo {
                path: cwd,
                name: project_name,
                bound_editor: Some(editor.version.clone()),
                last_opened: Some(now),
            };
            config.register_project(&project_info);
            config.save()?;
            return Ok(());
        }

        // No editor found - prompt user
        println!("Godot {} editor not found.", version);
        let options = vec![
            "Open with existing editor".to_string(),
            format!("Download Godot {}", version),
        ];

        let selection = dialoguer::FuzzySelect::new()
            .with_prompt("What would you like to do?")
            .items(&options)
            .default(0)
            .interact()?;

        match selection {
            0 => {
                let editors: Vec<_> = config.editors.values().cloned().collect();
                if editors.is_empty() {
                    println!("No editors installed. Use `gdio add` to install one.");
                    return Ok(());
                }
                let names: Vec<String> = editors.iter().map(|e| e.name.clone()).collect();
                let idx = dialoguer::FuzzySelect::new()
                    .with_prompt("Select editor")
                    .items(&names)
                    .default(0)
                    .interact()?;
                let editor = &editors[idx];
                godot::open_project_editor_mode(&editor.path, &project_file)?;

                let now = chrono_now();
                let editor_ver = editor.version.clone();
                let project_info = crate::config::ProjectInfo {
                    path: cwd,
                    name: project_name,
                    bound_editor: Some(editor_ver),
                    last_opened: Some(now),
                };
                config.register_project(&project_info);
                config.save()?;
            }
            1 => {
                let version = version.clone();
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(async {
                    let mut config = Config::load()?;
                    crate::commands::add::download_version_auto(&version, false, &mut config)
                        .await?;

                    if let Some(editor) = config.find_editor_for_version(&version) {
                        let project_file = std::env::current_dir()?.join("project.godot");
                        godot::open_project_editor_mode(&editor.path, &project_file)?;
                        let cwd = std::env::current_dir()?;
                        let project_name = project::parse_project_name(&project_file)
                            .unwrap_or_else(|| "Unknown Project".to_string());
                        let now = chrono_now();
                        let project_info = crate::config::ProjectInfo {
                            path: cwd,
                            name: project_name,
                            bound_editor: Some(editor.version.clone()),
                            last_opened: Some(now),
                        };
                        config.register_project(&project_info);
                        config.save()?;
                    }
                    Ok::<(), anyhow::Error>(())
                })?;
            }
            _ => {}
        }
    } else {
        println!("Could not determine Godot version from project.godot");
        println!("Use `gdio --help` for usage information.");
    }

    Ok(())
}

pub fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", secs)
}
