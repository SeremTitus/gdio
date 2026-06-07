use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub fn open_project_editor_mode(exe_path: &Path, project_path: &Path) -> Result<()> {
    let project_dir = project_path
        .parent()
        .context("Could not determine project directory")?;

    Command::new(exe_path)
        .args(["--path", &project_dir.to_string_lossy(), "--editor"])
        .spawn()
        .with_context(|| format!("Failed to launch editor: {}", exe_path.display()))?;
    Ok(())
}

pub fn open_project_game_mode(exe_path: &Path, project_path: &Path) -> Result<()> {
    let project_dir = project_path
        .parent()
        .context("Could not determine project directory")?;

    Command::new(exe_path)
        .args(["--path", &project_dir.to_string_lossy(), "--game"])
        .spawn()
        .with_context(|| format!("Failed to launch game: {}", exe_path.display()))?;
    Ok(())
}

pub fn open_headless_export(
    exe_path: &Path,
    project_path: &Path,
    preset: &str,
    output_path: &Path,
    debug: bool,
) -> Result<()> {
    let project_dir = project_path
        .parent()
        .context("Could not determine project directory")?;

    let export_flag = if debug {
        "--export-debug"
    } else {
        "--export-release"
    };

    let status = Command::new(exe_path)
        .args([
            "--headless",
            "--path",
            &project_dir.to_string_lossy(),
            export_flag,
            preset,
            &output_path.to_string_lossy(),
        ])
        .status()
        .with_context(|| format!("Failed to run export: {}", exe_path.display()))?;

    if !status.success() {
        anyhow::bail!("Export failed with exit code: {:?}", status.code());
    }
    Ok(())
}