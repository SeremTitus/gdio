use crate::config::Config;
use crate::godot;
use anyhow::{Context, Result};
use std::process::Command;

pub async fn run(
    url: &str,
    dir: Option<&str>,
    depth: Option<u32>,
    config: &mut Config,
) -> Result<()> {
    let project_dir = dir
        .map(|d| d.to_string())
        .unwrap_or_else(|| dir_from_url(url));

    if project_dir.is_empty() {
        anyhow::bail!("Could not determine directory name from URL. Provide a directory name.");
    }

    let cwd = std::env::current_dir().context("Failed to get current directory")?;

    if cwd.join("project.godot").exists() {
        let proceed = dialoguer::Confirm::new()
            .with_prompt("Current directory is already a Godot project. Clone here anyway?")
            .default(false)
            .interact()?;
        if !proceed {
            anyhow::bail!("Aborted.");
        }
    }

    let target = cwd.join(&project_dir);

    if target.exists() {
        anyhow::bail!(
            "Directory '{}' already exists. \
             Choose a different name or remove it first.",
            project_dir
        );
    }

    println!("Cloning {} into {}...", url, project_dir);

    let mut args = vec!["clone".to_string(), url.to_string(), project_dir.clone()];
    if let Some(d) = depth {
        args.insert(1, "--depth".to_string());
        args.insert(2, d.to_string());
    }

    let output = Command::new("git")
        .args(&args)
        .current_dir(&cwd)
        .output()
        .context("Failed to run git. Is git installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("{}", stderr.trim());
        anyhow::bail!(
            "git clone failed with exit code: {:?}",
            output.status.code()
        );
    }

    // Verify it's a Godot project
    let project_file = target.join("project.godot");
    if !project_file.exists() {
        anyhow::bail!(
            "Cloned repository does not contain a project.godot file. \
             This does not appear to be a Godot project."
        );
    }

    // Run editor detection from the cloned directory, restoring cwd on all paths
    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(&target)?;
    let result = open_project(&project_dir, config).await;
    let _ = std::env::set_current_dir(original_dir);
    result
}

async fn open_project(project_dir: &str, config: &mut Config) -> Result<()> {
    let ctx = super::shared::ProjectContext::detect(project_dir)?;
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
                godot::open_project_editor_mode(&editor.path, &ctx.project_file)?;
                super::shared::register_opened_project(
                    config,
                    ctx.cwd,
                    ctx.project_name,
                    &editor.version,
                )?;
            }
            1 => {
                let version = version.clone();
                crate::commands::add::download_version_auto(&version, false, config).await?;

                if let Some(editor) = config.find_editor_for_version(&version) {
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
            }
            _ => {}
        }
    } else {
        println!("Could not determine Godot version from project.godot");
        println!("Use `gdio --help` for usage information.");
    }

    Ok(())
}

fn dir_from_url(url: &str) -> String {
    let url = url.trim_end_matches('/');
    let url = url.strip_suffix(".git").unwrap_or(url);
    // Handle both https://host/user/repo and git@host:user/repo
    url.rsplit(|c| c == '/' || c == ':')
        .next()
        .unwrap_or(url)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dir_from_url() {
        assert_eq!(dir_from_url("https://github.com/user/repo.git"), "repo");
        assert_eq!(dir_from_url("https://github.com/user/repo"), "repo");
        assert_eq!(dir_from_url("https://github.com/user/repo/"), "repo");
        assert_eq!(dir_from_url("git@github.com:user/repo.git"), "repo");
        assert_eq!(dir_from_url("git@github.com:user/repo"), "repo");
        assert_eq!(
            dir_from_url("https://gitlab.com/group/subgroup/repo.git"),
            "repo"
        );
    }
}
