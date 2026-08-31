use crate::config::Config;
use crate::platform::PlatformFlags;
use crate::project;
use anyhow::{Context, Result};
use console::Style;
use futures_util::stream::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const BUTLER_RELEASES_URL: &str = "https://api.github.com/repos/itchio/butler/releases/latest";
const CHECK_INTERVAL_SECONDS: u64 = 24 * 60 * 60; // 24 hours

#[derive(Debug, Deserialize)]
struct ButlerRelease {
    tag_name: String,
    assets: Vec<ButlerAsset>,
}

#[derive(Debug, Deserialize)]
struct ButlerAsset {
    name: String,
    browser_download_url: String,
}

fn butler_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("gdio")
        .build()
        .context("Failed to build HTTP client")
}

fn platform_asset_name() -> Result<(&'static str, &'static str)> {
    if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
        Ok(("windows", "amd64"))
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        Ok(("linux", "amd64"))
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
        Ok(("linux", "arm64"))
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        Ok(("darwin", "amd64"))
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        Ok(("darwin", "arm64"))
    } else {
        anyhow::bail!(
            "Unsupported platform for butler download: {}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
    }
}

async fn ensure_butler(config: &mut Config) -> Result<PathBuf> {
    let butler_path = Config::get_butler_path();
    let butler_dir = Config::get_butler_dir();

    let installed_version = config
        .itch
        .as_ref()
        .and_then(|itch| itch.butler_version.clone());

    let last_checked = config
        .itch
        .as_ref()
        .and_then(|itch| itch.butler_last_checked);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let check_cached = last_checked
        .map(|ts| now.saturating_sub(ts) < CHECK_INTERVAL_SECONDS)
        .unwrap_or(false);

    if butler_path.exists() && !needs_download(&butler_path, &installed_version, check_cached) {
        return Ok(butler_path);
    }

    match download_butler(config, &butler_path, &butler_dir).await {
        Ok(path) => Ok(path),
        Err(download_err) => {
            eprintln!(
                "Warning: Failed to download butler: {}. Falling back to system butler.",
                download_err
            );
            find_system_butler()
                .context("Butler not found. Download failed and no system butler found on PATH.")
        }
    }
}

fn needs_download(
    butler_path: &Path,
    installed_version: &Option<String>,
    check_cached: bool,
) -> bool {
    if check_cached {
        return false;
    }
    if !butler_path.exists() {
        return true;
    }
    match installed_version {
        Some(_) => false, // version matches (already checked recently or same version)
        None => true,
    }
}

fn find_system_butler() -> Result<PathBuf> {
    let name = if cfg!(target_os = "windows") {
        "butler.exe"
    } else {
        "butler"
    };
    let output = Command::new(if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    })
    .arg(name)
    .output()
    .context("Failed to run which/where")?;

    if output.status.success() {
        let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let first_line = path_str.lines().next().unwrap_or(&path_str);
        let path = PathBuf::from(first_line);
        if path.exists() {
            return Ok(path);
        }
    }
    anyhow::bail!("butler not found on PATH")
}

async fn download_butler(
    config: &mut Config,
    butler_path: &Path,
    butler_dir: &Path,
) -> Result<PathBuf> {
    let client = butler_client()?;
    let release: ButlerRelease = client
        .get(BUTLER_RELEASES_URL)
        .send()
        .await
        .context("Failed to fetch butler release info")?
        .json()
        .await
        .context("Failed to parse butler release info")?;

    let needs = needs_download(
        butler_path,
        &config.itch.as_ref().and_then(|i| i.butler_version.clone()),
        false,
    );

    if !needs {
        return Ok(butler_path.to_path_buf());
    }

    println!("Downloading butler...");

    let (os, arch) = platform_asset_name()?;
    let zip_name = format!("butler-{}-{}.zip", os, arch);
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == zip_name)
        .with_context(|| format!("No butler asset found for {}-{}", os, arch))?;

    let downloads_dir = Config::get_downloads_dir();
    std::fs::create_dir_all(&downloads_dir)?;
    let zip_path = downloads_dir.join(&asset.name);

    let result = download_and_extract_butler(&client, asset, &zip_path, butler_dir, &release).await;

    let _ = std::fs::remove_file(&zip_path);

    result?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for entry in std::fs::read_dir(butler_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                std::fs::set_permissions(entry.path(), std::fs::Permissions::from_mode(0o755))
                    .with_context(|| {
                        format!("Failed to set permissions on {}", entry.path().display())
                    })?;
            }
        }
    }

    let itch_config = config.get_or_default_itch();
    itch_config.butler_version = Some(release.tag_name.clone());
    itch_config.butler_last_checked = Some(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    config.save()?;

    println!("  ✓ butler {} installed", release.tag_name);

    Ok(butler_path.to_path_buf())
}

async fn download_and_extract_butler(
    client: &reqwest::Client,
    asset: &ButlerAsset,
    zip_path: &Path,
    butler_dir: &Path,
    release: &ButlerRelease,
) -> Result<()> {
    let resp = client
        .get(&asset.browser_download_url)
        .send()
        .await
        .context("Failed to start butler download")?;

    if !resp.status().is_success() {
        anyhow::bail!("Butler download failed: HTTP {}", resp.status());
    }

    let total_size = resp.content_length().unwrap_or(0);
    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=>-"),
    );
    let size_mb = total_size as f64 / (1024.0 * 1024.0);
    pb.set_message(format!(
        "Downloading butler {} ({:.1} MB)",
        release.tag_name, size_mb
    ));

    let mut file = tokio::fs::File::create(zip_path)
        .await
        .context("Failed to create butler zip file")?;
    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Failed to read butler download chunk")?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .context("Failed to write butler download chunk")?;
        downloaded += chunk.len() as u64;
        pb.set_position(downloaded);
    }
    pb.finish_with_message(format!("Downloaded butler {}", release.tag_name));
    println!();

    drop(file);

    std::fs::create_dir_all(butler_dir)?;

    let zip_file = std::fs::File::open(zip_path)?;
    let mut archive =
        zip::ZipArchive::new(zip_file).context("Failed to open butler zip archive")?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let filename = entry
            .enclosed_name()
            .and_then(|p| p.file_name().map(|f| f.to_owned()))
            .context("Invalid file name in butler zip")?;
        let outpath = butler_dir.join(&filename);
        let mut outfile = std::fs::File::create(&outpath)?;
        std::io::copy(&mut entry, &mut outfile)?;
    }

    Ok(())
}

pub async fn run(
    platform: &PlatformFlags,
    debug: bool,
    name: bool,
    config: &mut Config,
) -> Result<()> {
    run_upload(platform, debug, name, config).await
}

pub async fn run_setup_with_game(game: &str, config: &mut Config) -> Result<()> {
    let ctx = super::shared::ProjectContext::detect("game")?;

    println!("=== itch.io upload setup ===\n");

    let butler_path = ensure_butler(config).await?;

    let butler_valid = Command::new(&butler_path).arg("--version").output().is_ok();
    if !butler_valid {
        anyhow::bail!(
            "Could not run butler at '{}'. Download may have failed.",
            butler_path.display()
        );
    }

    if !game.contains('/') {
        anyhow::bail!("Game identifier must be in 'user/game' format (e.g. 'myuser/mygame').");
    }

    let itch_config = config.get_or_default_itch();
    itch_config.set_project(
        &ctx.project_path,
        crate::config::ItchProjectConfig {
            game: game.to_string(),
        },
    );
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

async fn run_upload(
    platform: &PlatformFlags,
    debug: bool,
    name: bool,
    config: &mut Config,
) -> Result<()> {
    let ctx = super::shared::ProjectContext::detect("game")?;

    let butler_path = ensure_butler(config).await?;

    let itch_project = match config
        .itch
        .as_ref()
        .and_then(|itch| itch.get_project(&ctx.project_path))
    {
        Some(p) => p.clone(),
        None => {
            println!();
            let game: String = dialoguer::Input::new()
                .with_prompt("itch.io game identifier (user/game)")
                .interact_text()?;

            if !game.contains('/') {
                anyhow::bail!(
                    "Game identifier must be in 'user/game' format (e.g. 'myuser/mygame')."
                );
            }

            let itch_config = config.get_or_default_itch();
            itch_config.set_project(
                &ctx.project_path,
                crate::config::ItchProjectConfig { game: game.clone() },
            );
            config.save()?;

            crate::config::ItchProjectConfig { game }
        }
    };

    let game = &itch_project.game;

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

    let unique_platforms: Vec<String> = platform_presets.keys().cloned().collect();
    let platforms: Vec<String> = if platform.any() {
        platform.to_platforms()
    } else {
        unique_platforms
    };

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

    let editor = if let Some(editor) = ctx.bound_editor(config) {
        editor.clone()
    } else if let Some((version, editor)) = ctx.find_editor_for_detected_version(config) {
        println!("Detected Godot version: {}", version);
        editor.clone()
    } else {
        anyhow::bail!("No editor bound to this project. Use `gdio bind` to bind one.");
    };

    let game_version = project::parse_game_version(&ctx.project_file);
    let game_version_display = if game_version.is_empty() {
        let blue = Style::new().blue();
        println!(
            "{}",
            blue.apply_to("Project Settings: application/config/version is not set.")
        );
        let input: String = dialoguer::Input::new()
            .with_prompt("Game version for upload")
            .default("0.1.0-dev".to_string())
            .interact_text()?;
        input.trim().trim_start_matches('v').to_string()
    } else {
        game_version
    };

    let project_snake = super::shared::snake_case(&ctx.project_name);

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

    crate::godot::import_project(&editor.path, &ctx.project_file)?;

    println!("Building project for upload...\n");

    let output_dir = ctx.cwd.join("export");
    std::fs::create_dir_all(&output_dir)?;

    let mut exported: Vec<(String, PathBuf)> = Vec::new();

    for (preset_platform, preset) in &selected_presets {
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
            anyhow::bail!("Export failed for {}: {}", preset.name, e);
        }

        exported.push((preset_platform.clone(), output_file));
    }

    if exported.is_empty() {
        anyhow::bail!("No exports produced.");
    }

    println!("\nUploading to itch.io...\n");

    let temp_dir = std::env::temp_dir().join("gdio_upload");
    std::fs::create_dir_all(&temp_dir)?;

    for (platform, export_path) in &exported {
        let channel = channels.get(platform.as_str()).context(format!(
            "No channel configured for platform '{}'.",
            platform
        ))?;

        let parent_dir = export_path.parent().unwrap_or(&output_dir);
        let zip_name = format!(
            "{}-{}-v{}",
            ctx.project_name, platform, game_version_display
        );
        let zip_path = temp_dir.join(format!("{}.zip", zip_name));

        println!("  Zipping {}...", parent_dir.display());
        zip_dir(parent_dir, &zip_path)?;

        let target = format!("{}:{}", game, channel);
        println!("  Uploading {} → {}...", platform, target);

        let status = Command::new(&butler_path)
            .args([
                "push",
                &zip_path.to_string_lossy(),
                &target,
                "--userversion",
                &game_version_display,
            ])
            .status()
            .with_context(|| format!("Failed to run butler at '{}'", butler_path.display()))?;

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

        let _ = std::fs::remove_file(&zip_path);
    }

    let _ = std::fs::remove_dir_all(&temp_dir);

    println!("\nUpload complete.");
    Ok(())
}
