use crate::config::Config;
use crate::godot;
use crate::project;
use anyhow::Result;

pub async fn run(config: &mut Config) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let project_file = cwd.join("project.godot");

    if !project_file.exists() {
        anyhow::bail!(
            "No Godot project found in current directory.\nUse `gdio --help` for usage information."
        );
    }

    let project_path = cwd.to_string_lossy().to_string();
    let project_name =
        project::parse_project_name(&project_file).unwrap_or_else(|| "Unknown Project".to_string());

    println!("Found project: {} ({})", project_name, project_path);

    // Auto-sync addons if .gdio file exists
    let gdio_file = cwd.join(".gdio");
    if gdio_file.exists() {
        crate::commands::addons::sync::run_sync(config, &cwd).await?;
    }

    // Check if we've opened this project before
    if let Some(existing) = config.projects.get(&project_path)
        && let Some(ref editor_version) = existing.bound_editor
    {
        let editor_version = editor_version.clone();
        if let Some(editor) = config.find_editor_for_version(&editor_version)
            && editor.path.exists()
        {
            println!("Opening with {}...", editor.name);
            let path = project_file.clone();
            godot::open_project_editor_mode(&editor.path, &path)?;
            super::shared::register_opened_project(config, cwd, project_name, &editor_version)?;
            return Ok(());
        }
    }

    // Try to find editor for detected version
    let godot_version = project::parse_godot_version(&project_file);
    if let Some(ref version) = godot_version {
        println!("Project requires Godot {}", version);
        if let Some(editor) = config.find_editor_for_version(version)
            && editor.path.exists()
        {
            println!("Found editor: {}", editor.name);
            let path = project_file.clone();
            let editor_version = editor.version.clone();
            godot::open_project_editor_mode(&editor.path, &path)?;
            super::shared::register_opened_project(config, cwd, project_name, &editor_version)?;
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
                super::shared::register_opened_project(config, cwd, project_name, &editor.version)?;
            }
            1 => {
                let version = version.clone();
                crate::commands::add::download_version_auto(&version, false, config).await?;

                if let Some(editor) = config.find_editor_for_version(&version) {
                    let project_file = std::env::current_dir()?.join("project.godot");
                    let editor_path = editor.path.clone();
                    let editor_version = editor.version.clone();
                    godot::open_project_editor_mode(&editor_path, &project_file)?;
                    let cwd = std::env::current_dir()?;
                    let project_name = project::parse_project_name(&project_file)
                        .unwrap_or_else(|| "Unknown Project".to_string());
                    super::shared::register_opened_project(
                        config,
                        cwd,
                        project_name,
                        &editor_version,
                    )?;
                }
            }
            _ => {}
        }
    } else {
        println!("Could not determine Godot version from project.godot");
        println!("Use `gdio --help` for usage information.");
    }

    Ok(())
}
