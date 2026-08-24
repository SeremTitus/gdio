use crate::config::Config;
use crate::platform::PlatformFlags;
use crate::project;
use anyhow::{Context, Result};
use console::Style;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn detect_butler() -> Option<String> {
    let config_dir = dirs::config_dir()?;
    let broth_dir = config_dir.join("itch").join("broth").join("butler");

    let version_file = broth_dir.join(".chosen-version");
    if let Ok(version) = std::fs::read_to_string(&version_file) {
        let version = version.trim().to_string();
        if !version.is_empty() {
            let butler_path = if cfg!(target_os = "windows") {
                broth_dir.join("versions").join(&version).join("butler.exe")
            } else {
                broth_dir.join("versions").join(&version).join("butler")
            };

            if butler_path.exists() {
                return Some(butler_path.to_string_lossy().to_string());
            }
        }
    }

    if Command::new("butler").arg("--version").output().is_ok() {
        return Some("butler".to_string());
    }

    None
}

pub fn run(
    setup: bool,
    platform: &PlatformFlags,
    debug: bool,
    name: bool,
    config: &mut Config,
) -> Result<()> {
    if setup {
        return run_setup(config);
    }
    run_upload(platform, debug, name, config)
}

fn run_setup(config: &mut Config) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let project_file = cwd.join("project.godot");

    if !project_file.exists() {
        anyhow::bail!(
            "No Godot project found in current directory. Run this command from a directory containing a project.godot file."
        );
    }

    println!("=== itch.io upload setup ===\n");

    let default_butler = config
        .itch
        .as_ref()
        .map(|itch| itch.butler_path.clone())
        .or_else(detect_butler)
        .unwrap_or_else(|| "butler".to_string());
    let butler_path: String = dialoguer::Input::new()
        .with_prompt("Path to butler")
        .default(default_butler)
        .interact_text()?;

    let butler_path = butler_path
        .trim_matches(|c| c == '"' || c == '\'')
        .to_string();

    let butler_valid = Command::new(&butler_path).arg("--version").output().is_ok();
    if !butler_valid {
        anyhow::bail!(
            "Could not run butler at '{}'. Make sure it is installed and on your PATH.\n\
             Download butler from: https://itch.io/docs/butler/installing.html",
            butler_path
        );
    }
    println!("  ✓ butler found at '{}'\n", butler_path);

    let game: String = dialoguer::Input::new()
        .with_prompt("itch.io game identifier (user/game)")
        .interact_text()?;

    if !game.contains('/') {
        anyhow::bail!("Game identifier must be in 'user/game' format (e.g. 'myuser/mygame').");
    }

    let itch_config = config.get_or_default_itch();
    itch_config.butler_path = butler_path;

    let project_path = cwd.to_string_lossy().to_string();

    itch_config.set_project(&project_path, crate::config::ItchProjectConfig { game });
    config.save()?;

    println!("\n✓ itch.io upload configured for this project.");
    Ok(())
}

fn zip_dir(dir: &Path, zip_path: &Path) -> Result<()> {
    let file = std::fs::File::create(zip_path)
        .with_context(|| format!("Failed to create zip file {}", zip_path.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for entry in walkdir::WalkDir::new(dir).min_depth(1) {
        let entry = entry?;
        let path = entry.path();
        let name = path.strip_prefix(dir).unwrap_or(path);

        if path.is_file() {
            zip.start_file(name.to_string_lossy(), options)?;
            let mut f = std::fs::File::open(path)?;
            std::io::copy(&mut f, &mut zip)?;
        }
    }
    zip.finish()?;
    Ok(())
}

fn run_upload(platform: &PlatformFlags, debug: bool, name: bool, config: &Config) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let project_file = cwd.join("project.godot");

    if !project_file.exists() {
        anyhow::bail!("No Godot project found in current directory.");
    }

    let itch = config
        .itch
        .as_ref()
        .context("No itch.io configuration found. Run 'gdio up --setup' first.")?;

    let project_path = cwd.to_string_lossy().to_string();
    let itch_project = itch.get_project(&project_path).context(
        "This project is not configured for itch.io upload. Run 'gdio up --setup' first.",
    )?;

    let butler_path = &itch.butler_path;
    let game = &itch_project.game;

    // Parse presets to determine platforms
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

    // Group presets by platform
    let mut platform_presets: std::collections::HashMap<String, Vec<&project::ExportPreset>> =
        std::collections::HashMap::new();
    for preset in &presets {
        if let Some(p) = project::godot_platform_to_gdio(&preset.platform) {
            platform_presets
                .entry(p.to_string())
                .or_default()
                .push(preset);
        }
    }

    if platform_presets.is_empty() {
        anyhow::bail!("No valid platforms found in export presets.");
    }

    // Determine which platforms to build/upload
    let unique_platforms: Vec<String> = platform_presets.keys().cloned().collect();
    let platforms: Vec<String> = if platform.any() {
        platform.to_platforms()
    } else {
        unique_platforms
    };

    // For each selected platform, pick a preset (prompt if multiple)
    let mut selected_presets: Vec<(String, &project::ExportPreset)> = Vec::new();
    for platform in &platforms {
        let Some(presets_for) = platform_presets.get(platform) else {
            println!("Skipping {} - no export preset found", platform);
            continue;
        };
        let preset = if presets_for.len() == 1 {
            presets_for[0]
        } else {
            let names: Vec<String> = presets_for.iter().map(|p| p.name.clone()).collect();
            let idx = dialoguer::Select::new()
                .with_prompt(format!("Multiple presets for {}, select one", platform))
                .items(&names)
                .default(0)
                .interact()?;
            presets_for[idx]
        };
        selected_presets.push((platform.clone(), preset));
    }

    // Export
    let editor = if let Some(editor_version) = config
        .projects
        .get(&project_path)
        .and_then(|p| p.bound_editor.as_ref())
    {
        config
            .find_editor_for_version(editor_version)
            .cloned()
            .context(format!(
                "Bound editor '{}' not found. Use `gdio bind` to rebind.",
                editor_version
            ))?
    } else if let Some(version) = project::parse_godot_version(&project_file) {
        config
            .find_editor_for_version(&version)
            .cloned()
            .context(format!(
                "No editor bound and no editor found for Godot {}. Use `gdio bind` to bind one.",
                version
            ))?
    } else {
        anyhow::bail!("No editor bound to this project. Use `gdio bind` to bind one.");
    };

    let game_version = project::parse_game_version(&project_file);
    let game_version_display = if game_version.is_empty() {
        let blue = Style::new().blue();
        println!(
            "{}",
            blue.apply_to("Project Settings: application/config/version is not set.")
        );
        let input: String = dialoguer::Input::new()
            .with_prompt("Set game version")
            .default("0.1.0-dev".to_string())
            .interact_text()?;
        let version = input.trim().trim_start_matches('v').to_string();
        if !version.is_empty() {
            let content = std::fs::read_to_string(&project_file)?;
            let new_line = format!("config/version=\"{}\"", version);
            let mut lines: Vec<String> = content.lines().map(ToString::to_string).collect();
            let mut found = false;
            for line in &mut lines {
                if line.trim().starts_with("config/version=") {
                    line.clone_from(&new_line);
                    found = true;
                    break;
                }
            }
            if !found {
                lines.push(new_line);
            }
            std::fs::write(&project_file, lines.join("\n"))?;
        }
        version
    } else {
        game_version
    };

    let project_name =
        project::parse_project_name(&project_file).unwrap_or_else(|| "game".to_string());
    let project_snake = super::shared::snake_case(&project_name);

    // Determine channel names for each platform
    let mut channels: HashMap<String, String> = HashMap::new();

    for platform in &platforms {
        let default_channel = format!("{}-v{}", platform, game_version_display);
        let channel = if name {
            let input: String = dialoguer::Input::new()
                .with_prompt(format!("Channel name for {}", platform))
                .default(default_channel.clone())
                .interact_text()?;
            input.replace(['/', '\\'], "_")
        } else {
            default_channel
        };
        channels.insert(platform.clone(), channel);
    }

    if !editor.path.exists() {
        anyhow::bail!(
            "Godot editor not found at '{}'. Re-add it with 'gdio add'.",
            editor.path.display()
        );
    }

    println!("Building project for upload...\n");

    let output_dir = cwd.join("export");
    std::fs::create_dir_all(&output_dir)?;

    // Track exported paths per platform for zipping
    let mut exported: Vec<(String, PathBuf)> = Vec::new();

    for (preset_platform, preset) in &selected_presets {
        println!("Exporting preset: {} ({})", preset.name, preset_platform);

        let output_file = if let Some(ref export_path) = preset.export_path {
            cwd.join(export_path)
        } else if preset_platform == "web" {
            let preset_snake = super::shared::snake_case(&preset.name);
            output_dir.join(&preset_snake).join("index.html")
        } else if preset_platform == "linux" {
            let preset_snake = super::shared::snake_case(&preset.name);
            let arch = preset.binary_format.as_deref().unwrap_or("x86_64");
            output_dir
                .join(&preset_snake)
                .join(format!("{}.{}", project_snake, arch))
        } else {
            let preset_snake = super::shared::snake_case(&preset.name);
            let ext = match preset_platform.as_str() {
                "windows" => ".exe",
                "macos" => ".app",
                "ios" => ".ipa",
                "visionos" => ".ipa",
                "android" => ".apk",
                _ => "",
            };
            output_dir
                .join(&preset_snake)
                .join(format!("{}{}", project_snake, ext))
        };

        if let Some(parent) = output_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        println!("  → {}", output_file.display());

        if let Err(e) = crate::godot::open_headless_export(
            &editor.path,
            &project_file,
            &preset.name,
            &output_file,
            debug,
        ) {
            anyhow::bail!("Export failed for {}: {}", preset.name, e);
        }

        exported.push((preset_platform.clone(), output_file));
    }

    if exported.is_empty() {
        anyhow::bail!("No exports produced.");
    }

    // Zip and upload
    println!("\nUploading to itch.io...\n");

    let temp_dir = std::env::temp_dir().join("gdio_upload");
    std::fs::create_dir_all(&temp_dir)?;

    for (platform, export_path) in &exported {
        let channel = channels.get(platform.as_str()).context(format!(
            "No channel configured for platform '{}'.",
            platform
        ))?;

        let parent_dir = export_path.parent().unwrap_or(&output_dir);
        let zip_name = format!("{}-{}-v{}", project_name, platform, game_version_display);
        let zip_path = temp_dir.join(format!("{}.zip", zip_name));

        println!("  Zipping {}...", parent_dir.display());
        zip_dir(parent_dir, &zip_path)?;

        let target = format!("{}:{}", game, channel);
        println!("  Uploading {} → {}...", platform, target);

        let status = Command::new(butler_path)
            .args([
                "push",
                &zip_path.to_string_lossy(),
                &target,
                "--userversion",
                &game_version_display,
            ])
            .status()
            .with_context(|| format!("Failed to run butler at '{}'", butler_path))?;

        if !status.success() {
            let red = Style::new().red();
            eprintln!(
                "{}",
                red.apply_to(format!(
                    "  ✗ butler push failed for {} (exit code: {:?})",
                    platform,
                    status.code()
                ))
            );
        } else {
            println!("  ✓ {} uploaded", platform);
        }

        // Clean up zip
        let _ = std::fs::remove_file(&zip_path);
    }

    let _ = std::fs::remove_dir_all(&temp_dir);

    println!("\nUpload complete.");
    Ok(())
}
