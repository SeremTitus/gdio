use crate::config::Config;
use anyhow::{Context, Result};
use std::process::Command;

pub async fn run(file: &str, config: &mut Config) -> Result<()> {
    let ctx = super::shared::ProjectContext::detect("Unknown Project").ok();

    let editor = if let Some(ref ctx) = ctx {
        ctx.bound_editor(config)
            .or_else(|| ctx.find_editor_for_detected_version(config).map(|(_, e)| e))
            .cloned()
    } else {
        None
    };

    let editor = match editor {
        Some(e) => e,
        None => super::shared::resolve_editor(config, None).await?,
    };

    if let Some(ref ctx) = ctx {
        println!("Running {} script: {}...", ctx.project_name, file);
    } else {
        println!("Running script: {}...", file);
    }

    let mut cmd = Command::new(&editor.path);
    cmd.args(["--headless", "--script", file]);

    let status = cmd
        .status()
        .with_context(|| format!("Failed to launch editor: {}", editor.path.display()))?;

    if !status.success() {
        anyhow::bail!("Script failed (exit code: {:?}).", status.code());
    }

    Ok(())
}
