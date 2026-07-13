use anyhow::{Context, Result};
use crate::config::Config;
use crate::project;
use console::Style;

#[allow(clippy::too_many_arguments)]
pub fn run(
    windows: bool,
    linux: bool,
    web: bool,
    macos: bool,
    ios: bool,
    android: bool,
    debug: bool,
    config: &mut Config,
) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let project_file = cwd.join("project.godot");

    if !project_file.exists() {
        anyhow::bail!("No Godot project found in current directory.");
    }

    let presets_file = cwd.join("export_presets.cfg");
    if !presets_file.exists() {
        anyhow::bail!(
            "No export_presets.cfg found. Create export presets in the Godot editor first."
        );
    }

    let presets = project::parse_export_presets(&presets_file);
    if presets.is_empty() {
        anyhow::bail!("No export presets found in export_presets.cfg.");
    }

    let has_flag = windows || linux || web || macos || ios || android;
    let mut platforms = Vec::new();
    if has_flag {
        if windows { platforms.push("windows".to_string()); }
        if linux { platforms.push("linux".to_string()); }
        if web { platforms.push("web".to_string()); }
        if macos { platforms.push("macos".to_string()); }
        if ios { platforms.push("ios".to_string()); }
        if android { platforms.push("android".to_string()); }
    } else {
        let mut seen = std::collections::HashSet::new();
        for preset in &presets {
            if let Some(gdio_platform) = project::godot_platform_to_gdio(&preset.platform)
                && seen.insert(gdio_platform.to_string())
            {
                platforms.push(gdio_platform.to_string());
            }
        }
        if platforms.is_empty() {
            anyhow::bail!(
                "Could not detect platforms from export presets. Use platform flags explicitly."
            );
        }
        println!("Detected platforms from presets: {}\n", platforms.join(", "));
    }

    let godot_version = project::parse_godot_version(&project_file)
        .context("Could not determine Godot version from project.godot")?;

    let project_name = project::parse_project_name(&project_file)
        .unwrap_or_else(|| "game".to_string());
    let project_snake: String = project_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect();

    let editor = config
        .find_editor_for_version(&godot_version)
        .context(format!("No editor found for Godot {}", godot_version))?
        .clone();

    let mut output_paths = Vec::new();

    for preset in &presets {
        let preset_platform = match project::godot_platform_to_gdio(&preset.platform) {
            Some(p) => p,
            None => {
                let yellow = Style::new().yellow();
                eprintln!("{}", yellow.apply_to(format!("Skipping preset '{}' - unknown platform '{}'", preset.name, preset.platform)));
                continue;
            }
        };

        if !platforms.contains(&preset_platform.to_string()) {
            continue;
        }

        println!("Exporting preset: {} ({})", preset.name, preset_platform);

        let output_dir = cwd.join("export");

        let output_file = if let Some(ref export_path) = preset.export_path {
            cwd.join(export_path)
        } else {
            let preset_snake: String = preset.name
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
                .collect();
            if preset_platform == "web" {
                output_dir.join(&preset_snake).join("index.html")
            } else if preset_platform == "linux" {
                let arch = preset.binary_format.as_deref().unwrap_or("x86_64");
                output_dir.join(&preset_snake).join(format!("{}.{}", project_snake, arch))
            } else {
                let ext = match preset_platform {
                    "windows" => ".exe",
                    "macos" => ".app",
                    "ios" => ".ipa",
                    "android" => ".apk",
                    _ => "",
                };
                output_dir.join(&preset_snake).join(format!("{}{}", project_snake, ext))
            }
        };

        if let Some(parent) = output_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        println!("  -> {}", output_file.display());

        if let Err(e) = crate::godot::open_headless_export(
            &editor.path,
            &project_file,
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
