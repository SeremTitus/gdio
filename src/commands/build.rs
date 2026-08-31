use crate::config::{self, Config};
use crate::platform::PlatformFlags;
use crate::project;
use anyhow::{Context, Result};
use console::Style;

pub async fn run(platform: &PlatformFlags, debug: bool, config: &Config) -> Result<()> {
    let ctx = super::shared::ProjectContext::detect("game")?;

    let presets_file = ctx.cwd.join("export_presets.cfg");
    if !presets_file.exists() {
        anyhow::bail!(
            "No export_presets.cfg found. Create export presets in the Godot editor first."
        );
    }

    let presets = project::parse_export_presets(&presets_file);
    if presets.is_empty() {
        anyhow::bail!("No export presets found in export_presets.cfg.");
    }

    let platforms = if platform.any() {
        platform.to_platforms()
    } else {
        // No flags: auto-detect from export presets
        let mut detected = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for preset in &presets {
            if let Some(gdio_platform) = project::godot_platform_to_gdio(&preset.platform)
                && seen.insert(gdio_platform.to_string())
            {
                detected.push(gdio_platform.to_string());
            }
        }
        if detected.is_empty() {
            anyhow::bail!(
                "Could not detect platforms from export presets. Use platform flags explicitly."
            );
        }
        println!("Detected platforms from presets: {}\n", detected.join(", "));
        detected
    };

    let project_snake = super::shared::snake_case(&ctx.project_name);

    let editor_version = config
        .projects
        .get(&ctx.project_path)
        .and_then(|p| p.bound_editor.clone())
        .context("No editor bound to this project. Use `gdio bind` to bind one.")?;
    let editor = config
        .find_editor_for_version(&editor_version)
        .context(format!(
            "Bound editor '{}' not found. Use `gdio bind` to rebind.",
            editor_version
        ))?
        .clone();
    let godot_version = editor.version.clone();

    crate::godot::import_project(&editor.path, &ctx.project_file)?;

    for preset in &presets {
        let preset_platform = match project::godot_platform_to_gdio(&preset.platform) {
            Some(p) => p,
            None => {
                continue;
            }
        };

        if !platforms.contains(&preset_platform.to_string()) {
            continue;
        }

        ensure_templates(&godot_version, preset_platform).await?;
    }

    let mut output_paths = Vec::new();
    let output_dir = ctx.cwd.join("export");

    for preset in &presets {
        let preset_platform = match project::godot_platform_to_gdio(&preset.platform) {
            Some(p) => p,
            None => {
                let yellow = Style::new().yellow();
                eprintln!(
                    "{}",
                    yellow.apply_to(format!(
                        "Skipping preset '{}' - unknown platform '{}'",
                        preset.name, preset.platform
                    ))
                );
                continue;
            }
        };

        if !platforms.contains(&preset_platform.to_string()) {
            continue;
        }

        println!("Exporting preset: {} ({})", preset.name, preset_platform);

        let output_file = super::shared::compute_export_output_path(
            &ctx.cwd,
            &output_dir,
            &project_snake,
            preset_platform,
            preset,
        );

        if let Some(parent) = output_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        println!("  → {}", output_file.display());

        if let Err(e) = crate::godot::open_headless_export(
            &editor.path,
            &ctx.project_file,
            &preset.name,
            &output_file,
            debug,
        ) {
            let red = Style::new().red();
            eprintln!("{}", red.apply_to(format!("  Export failed: {}", e)));
        } else {
            output_paths.push(output_file);
        }
    }

    if output_paths.is_empty() {
        anyhow::bail!("No exports produced.");
    }

    println!("\nExport complete.");
    for path in &output_paths {
        println!("  {}", path.display());
    }
    Ok(())
}

async fn ensure_templates(version: &str, platform: &str) -> Result<()> {
    let (base_version, flavor) = config::parse_version_flavor(version);
    let godot_dir = Config::get_godot_templates_dir().join(format!("{}.{}", base_version, flavor));

    if godot_dir.exists() {
        let platforms = crate::commands::templates::list::detect_platforms(godot_dir.as_path())?;
        if platforms.contains(&platform.to_string()) {
            return Ok(());
        }
    }

    println!("Export templates for {} {} not found.", version, platform);
    let download = dialoguer::Confirm::new()
        .with_prompt(format!(
            "Download {} templates for Godot {}?",
            platform, version
        ))
        .default(true)
        .interact()?;

    if download {
        std::fs::create_dir_all(&godot_dir)?;
        // Use templates add instead
        let client = reqwest::Client::builder().user_agent("gdio").build()?;
        crate::commands::templates::api::download_template_files(
            &client,
            version,
            platform,
            godot_dir.as_path(),
        )
        .await?;
    }

    Ok(())
}
