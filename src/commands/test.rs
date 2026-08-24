use crate::config::Config;
use anyhow::{Context, Result};

pub async fn run(
    init: bool,
    visual: bool,
    folder: Option<&str>,
    config: &mut Config,
) -> Result<()> {
    if init {
        return run_init(config).await;
    }

    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let project_file = cwd.join("project.godot");

    if !project_file.exists() {
        anyhow::bail!("No Godot project found in current directory.");
    }

    let gut_script = cwd.join("addons").join("gut").join("gut_cmdln.gd");
    if !gut_script.exists() {
        anyhow::bail!("GUT not found. Install it with `gdio test --init`.");
    }

    let project_path = cwd.to_string_lossy().to_string();
    let editor = config
        .projects
        .get(&project_path)
        .and_then(|p| p.bound_editor.as_ref())
        .and_then(|v| config.find_editor_for_version(v))
        .context("No editor bound to this project. Use `gdio bind` to bind one.")?
        .clone();

    let mut args = Vec::new();

    if !visual {
        args.push("--headless".to_string());
    }

    args.push("--path".to_string());
    args.push(cwd.to_string_lossy().to_string());
    args.push("-s".to_string());
    args.push("addons/gut/gut_cmdln.gd".to_string());

    match folder {
        Some(f) => {
            let f = f.trim_start_matches('/').trim_start_matches('\\');
            args.push(format!("-gdir=res://{}", f));
        }
        None if !cwd.join(".gutconfig.json").exists() => {
            args.push("-gdir=res://".to_string());
            args.push("-ginclude_subdirs".to_string());
        }
        _ => {}
    }

    if visual {
        args.push("-gcompact_mode".to_string());
    } else {
        args.push("-gexit".to_string());
    }

    let status = std::process::Command::new(&editor.path)
        .args(&args)
        .status()
        .with_context(|| format!("Failed to run Godot: {}", editor.path.display()))?;

    if !status.success() {
        anyhow::bail!("Tests failed (exit code: {:?}).", status.code());
    }

    Ok(())
}

async fn run_init(_config: &mut Config) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let project_file = cwd.join("project.godot");

    if !project_file.exists() {
        anyhow::bail!("Not in a Godot project directory.");
    }

    let gut_script = cwd.join("addons").join("gut").join("gut_cmdln.gd");
    if gut_script.exists() {
        println!("GUT is already installed.");
        return Ok(());
    }

    println!("Fetching GUT releases...");

    let client = reqwest::Client::builder().user_agent("gdio").build()?;

    let releases = crate::commands::addons::api::fetch_releases(
        &client,
        "https://store.godotengine.org",
        "bitwes",
        "gut",
    )
    .await?;

    let release = releases
        .iter()
        .filter(|r| r.stable)
        .max_by_key(|r| {
            let v = &r.version;
            let parts: Vec<&str> = v.split('.').collect();
            let major = parts
                .first()
                .map(|s| s.parse::<u32>().unwrap_or(0) * 10000)
                .unwrap_or(0);
            let minor = parts
                .get(1)
                .map(|s| s.parse::<u32>().unwrap_or(0) * 100)
                .unwrap_or(0);
            let patch = parts
                .get(2)
                .map(|s| s.parse::<u32>().unwrap_or(0))
                .unwrap_or(0);
            major + minor + patch
        })
        .or_else(|| releases.first())
        .context("No releases found for bitwes/gut")?;

    let cache_dir = Config::get_addons_cache_dir();
    let zip_name = format!("bitwes_gut_v{}.zip", release.version);

    println!("Downloading GUT v{}...", release.version);
    let zip_path = crate::commands::addons::api::download_zip(
        &client,
        &release.download_url,
        &cache_dir,
        &zip_name,
    )
    .await?;

    println!("Installing to project...");
    let folder_name = crate::commands::addons::storage::extract_addon(&zip_path, &cwd, false)?;
    let _ = std::fs::remove_file(&zip_path);

    crate::commands::addons::storage::enable_plugin(&cwd, &folder_name)?;

    println!("Installed GUT v{} as {}", release.version, folder_name);
    Ok(())
}
