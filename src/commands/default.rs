use crate::config::Config;
use crate::godot;
use anyhow::Result;

pub async fn run(config: &mut Config) -> Result<()> {
    let ctx = super::shared::ProjectContext::detect("Unknown Project")?;

    println!("Found project: {} ({})", ctx.project_name, ctx.project_path);

    // Auto-sync addons if .gdio file exists
    let gdio_file = ctx.cwd.join(".gdio");
    if gdio_file.exists() {
        crate::commands::addons::sync::run_sync(config, &ctx.cwd).await?;
    }

    // Check if we've opened this project before
    if let Some(editor) = ctx.bound_editor(config)
        && editor.path.exists()
    {
        println!("Opening with {}...", editor.name);
        let editor_path = editor.path.clone();
        let editor_version = editor.version.clone();
        godot::open_project_editor_mode(&editor_path, &ctx.project_file)?;
        super::shared::register_opened_project(config, ctx.cwd, ctx.project_name, &editor_version)?;
        return Ok(());
    }

    // Try to find editor for detected version
    if let Some((version, editor)) = ctx.find_editor_for_detected_version(config)
        && editor.path.exists()
    {
        println!("Project requires Godot {}", version);
        println!("Found editor: {}", editor.name);
        let editor_path = editor.path.clone();
        let editor_version = editor.version.clone();
        godot::open_project_editor_mode(&editor_path, &ctx.project_file)?;
        super::shared::register_opened_project(config, ctx.cwd, ctx.project_name, &editor_version)?;
        return Ok(());
    }

    // No editor found - prompt user
    let godot_version = crate::project::parse_godot_version(&ctx.project_file);
    if let Some(ref version) = godot_version {
        println!("Project requires Godot {}", version);
        println!("Godot {} editor not found. Downloading...", version);
        crate::commands::add::download_version_auto(version, false, config).await?;

        if let Some(editor) = config.find_editor_for_version(version) {
            let editor_path = editor.path.clone();
            let editor_version = editor.version.clone();
            godot::open_project_editor_mode(&editor_path, &ctx.project_file)?;
            super::shared::register_opened_project(
                config,
                ctx.cwd,
                ctx.project_name,
                &editor_version,
            )?;
        }
    } else {
        println!("Could not determine Godot version from project.godot");
        println!("Use `gdio --help` for usage information.");
    }

    Ok(())
}
