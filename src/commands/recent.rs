use crate::config::Config;
use crate::godot;
use anyhow::Result;

pub async fn run(config: &mut Config) -> Result<()> {
    if config.projects.is_empty() {
        println!("No projects registered.");
        println!("Use `gdio` in a project directory to register it.");
        return Ok(());
    }

    super::shared::cleanup_missing_projects(config)?;

    let mut projects: Vec<_> = config.projects.values().cloned().collect();
    projects.sort_by(|a, b| b.last_opened.cmp(&a.last_opened));

    let recent = match projects.into_iter().next() {
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

    let editor = super::shared::resolve_editor(config, recent.bound_editor.as_deref()).await?;

    godot::open_project_editor_mode(&editor.path, &project_file)?;
    super::shared::register_opened_project(config, recent.path, recent.name, &editor.version)?;
    Ok(())
}
