use anyhow::Result;
use console::Style;
use crate::config::{self, Config, EditorInfo, EditorSource};
use crate::github;
use std::path::PathBuf;

pub fn run(version: &str, path: Option<&str>, csharp: bool, config: &mut Config) -> Result<Option<EditorInfo>> {
    if let Some(local_path) = path {
        return register_local(local_path, config);
    }

    if looks_like_path(version) {
        return register_local(version, config);
    }

    let (ver, stage) = parse_version_arg(version);
    let rt = tokio::runtime::Runtime::new()?;
    if let Some(stage) = stage {
        rt.block_on(download_version(&ver, &stage, csharp, config))
    } else {
        rt.block_on(download_version_auto(&ver, csharp, config))
    }
}

fn looks_like_path(s: &str) -> bool {
    s.contains('/') || s.contains('\\')
}

pub fn parse_version_arg(arg: &str) -> (String, Option<String>) {
    let (ver, flavor) = config::parse_version_flavor(arg);
    if flavor == "stable" {
        // No explicit flavor specified
        (arg.to_string(), None)
    } else {
        (ver.to_string(), Some(flavor.to_string()))
    }
}

fn register_downloaded_editor(
    exe_path: PathBuf,
    version_key: &str,
    is_mono: bool,
    csharp: bool,
    config: &mut Config,
) -> Result<Option<EditorInfo>> {
    if let Some(existing) = config.find_editor_for_version(version_key) {
        println!(
            "Already exists: {} ({})",
            existing.name,
            existing.path.display()
        );
        return Ok(None);
    }

    let editor_name = if csharp {
        format!("Godot v{}-csharp", version_key)
    } else {
        format!("Godot v{}", version_key)
    };

    let editor = EditorInfo {
        name: editor_name,
        path: exe_path,
        version: version_key.to_string(),
        is_mono: csharp || is_mono,
        source: EditorSource::Downloaded,
    };

    println!("Registered: {} ({})", editor.name, editor.path.display());
    config.register_editor(editor.clone());
    config.save()?;
    Ok(Some(editor))
}

pub async fn download_version(
    version: &str,
    stage: &str,
    csharp: bool,
    config: &mut Config,
) -> Result<Option<EditorInfo>> {
    let version_key = format!("{}-{}", version, stage);
    let editors_dir = Config::get_editors_dir();
    let suffix = if csharp {
        github::mono_dir_suffix()
    } else {
        github::platform_dir_suffix()
    };
    let dir_name = format!("Godot_v{}-{}_{}", version, stage, suffix);
    let dest_dir = editors_dir.join(&dir_name);

    // Check if executable already exists on disk
    if let Ok(exe_path) = github::find_executable_in_dir(&dest_dir) {
        println!("Already exists: {}", exe_path.display());
        if let Some(existing) = config.find_editor_for_version(&version_key) {
            println!("Already registered: {} ({})", existing.name, existing.path.display());
            return Ok(None);
        }
        return register_downloaded_editor(exe_path, &version_key, false, csharp, config);
    }

    let (exe_path, _stage) =
        github::download_and_extract_editor(version, stage, csharp, &dest_dir).await?;

    register_downloaded_editor(exe_path, &version_key, false, csharp, config)
}

pub async fn download_version_auto(
    version: &str,
    csharp: bool,
    config: &mut Config,
) -> Result<Option<EditorInfo>> {
    let editors_dir = Config::get_editors_dir();
    let suffix = if csharp {
        github::mono_dir_suffix()
    } else {
        github::platform_dir_suffix()
    };
    let dir_name = format!("Godot_v{}_{}", version, suffix);
    let dest_dir = editors_dir.join(&dir_name);

    // Check if executable already exists on disk
    if let Ok(exe_path) = github::find_executable_in_dir(&dest_dir) {
        println!("Already exists: {}", exe_path.display());
        // Determine the actual stage for this version from GitHub
        let stage = match github::fetch_release_auto(version).await {
            Ok((_, stage)) => stage,
            Err(_) => {
                // API failed — try to find an already-registered editor for this version prefix
                if let Some(existing) = config.find_editor_for_version(version) {
                    println!("Already registered: {} ({})", existing.name, existing.path.display());
                    return Ok(None);
                }
                // API unavailable and no registered editor — default to "stable"
                eprintln!("Warning: GitHub API unavailable, defaulting to 'stable' stage for '{}'", version);
                "stable".to_string()
            }
        };
        let version_key = format!("{}-{}", version, stage);
        // Try to find and register if not in config
        if let Some(existing) = config.find_editor_for_version(&version_key) {
            println!("Already registered: {} ({})", existing.name, existing.path.display());
            return Ok(None);
        }
        // Register existing executable
        return register_downloaded_editor(exe_path, &version_key, false, csharp, config);
    }

    let (exe_path, stage) =
        github::download_and_extract_editor_auto(version, csharp, &dest_dir).await?;

    let version_key = format!("{}-{}", version, stage);
    register_downloaded_editor(exe_path, &version_key, false, csharp, config)
}

fn register_local(path_str: &str, config: &mut Config) -> Result<Option<EditorInfo>> {
    let path_str = path_str.trim_matches(|c| c == '"' || c == '\'');
    let path = PathBuf::from(path_str);
    if !path.exists() {
        anyhow::bail!("File not found: {}", path_str);
    }

    let default_name = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let editor_name = loop {
        let input: String = dialoguer::Input::new()
            .with_prompt("Editor name")
            .default(default_name.clone())
            .interact_text()?;

        if let Some(existing) = config.editors.values().find(|e| e.name == input) {
            let source = if existing.source == EditorSource::Local { "local" } else { "downloaded" };
            println!(
                "{}",
                Style::new().blue().apply_to(format!(
                    "'{}' taken by {} ({})",
                    input, source, existing.path.display()
                ))
            );
            continue;
        }
        break input;
    };

    let editor = EditorInfo {
        name: editor_name,
        path: path.clone(),
        version: "local".to_string(),
        is_mono: false,
        source: EditorSource::Local,
    };

    println!("Registered: {} ({})", editor.name, path.display());
    config.register_editor(editor.clone());
    config.save()?;
    Ok(Some(editor))
}
