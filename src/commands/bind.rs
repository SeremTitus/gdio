use crate::config::Config;
use crate::project;
use anyhow::{Context, Result};

pub async fn run(target: Option<&str>, config: &mut Config) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let project_file = cwd.join("project.godot");

    if !project_file.exists() {
        anyhow::bail!("No Godot project found in current directory.");
    }

    let project_path = cwd.to_string_lossy().to_string();
    let project_name =
        project::parse_project_name(&project_file).unwrap_or_else(|| "Unknown Project".to_string());

    let selected_editor = match target {
        Some(name) => {
            // Try to find existing editor by version key or name
            let found = config.editors.iter().find(|(k, e)| {
                k.contains(name)
                    || e.name.to_lowercase().contains(&name.to_lowercase())
                    || e.version.to_lowercase().contains(&name.to_lowercase())
            });

            if let Some((_, editor)) = found {
                editor.clone()
            } else {
                // Editor not found - add it first
                println!("Editor '{}' not found, downloading...", name);
                let (ver, stage) = crate::commands::add::parse_version_arg(name);
                if let Some(stage) = stage {
                    crate::commands::add::download_version(&ver, &stage, false, config).await?;
                } else {
                    crate::commands::add::download_version_auto(&ver, false, config).await?;
                }
                // Find the editor we just added
                config
                    .editors
                    .iter()
                    .find(|(k, e)| {
                        k.contains(&ver) || e.name.to_lowercase().contains(&ver.to_lowercase())
                    })
                    .map(|(_, e)| e.clone())
                    .context("Failed to find newly added editor")?
            }
        }
        None => {
            // Interactive selection
            if config.editors.is_empty() {
                println!("No editors installed. Use `gdio add` to install one first.");
                return Ok(());
            }

            println!("Project: {}", project_name);

            let editors: Vec<_> = config.editors.values().cloned().collect();
            let names: Vec<String> = editors.iter().map(|e| e.name.clone()).collect();

            let idx = dialoguer::FuzzySelect::new()
                .with_prompt("Select editor to bind to this project")
                .items(&names)
                .default(0)
                .interact()?;

            editors[idx].clone()
        }
    };

    config.update_project_editor(&project_path, &selected_editor.version);

    super::shared::register_opened_project(config, cwd, project_name, &selected_editor.version)?;

    println!("Bound project to: {}", selected_editor.name);
    Ok(())
}
