use crate::config::Config;
use anyhow::{Context, Result};
use std::process::Command;

pub fn run(extra_args: &[String], config: &Config) -> Result<()> {
    let ctx = super::shared::ProjectContext::detect("Unknown Project")?;

    let editor = ctx
        .bound_editor(config)
        .or_else(|| ctx.find_editor_for_detected_version(config).map(|(_, e)| e))
        .context("No editor found for this project.\nUse `gdio add` to install an editor, then `gdio bind` to bind it.")?;

    println!("Running {} with extra args...", ctx.project_name);

    Command::new(&editor.path)
        .args(["--path", &ctx.cwd.to_string_lossy()])
        .args(extra_args)
        .spawn()
        .with_context(|| format!("Failed to launch editor: {}", editor.path.display()))?;

    Ok(())
}
