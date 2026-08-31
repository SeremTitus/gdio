use crate::config::Config;
use crate::godot;
use anyhow::{Context, Result};
use std::fs;

pub async fn run(name: &str, config: &mut Config) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;

    if cwd.join("project.godot").exists() {
        anyhow::bail!(
            "Current directory is already a Godot project. \
             Create new projects from a parent directory."
        );
    }

    let project_dir = cwd.join(name);

    if project_dir.exists() {
        anyhow::bail!("Directory '{}' already exists", name);
    }

    // Create project directory
    fs::create_dir_all(&project_dir)
        .with_context(|| format!("Failed to create directory: {}", project_dir.display()))?;

    // Select editor and open
    let editor = super::shared::resolve_editor(config, None).await?;

    let project_file = project_dir.join("project.godot");
    let content = format!(
        r#"[application]
config/name="{}"
"#,
        name,
    );
    fs::write(&project_file, content).context("Failed to write project.godot")?;

    println!("Created project: {}", project_dir.display());

    println!("Opening with {}...", editor.name);
    godot::open_project_editor_mode(&editor.path, &project_file)?;

    // Register project
    super::shared::register_opened_project(config, project_dir, name.to_string(), &editor.version)?;

    Ok(())
}
