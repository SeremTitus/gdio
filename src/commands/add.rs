use anyhow::{Context, Result};
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

fn resolve_editor_name(display_name: &str, config: &Config) -> Result<String> {
    if let Some(existing) = config.editors.values().find(|e| e.name == display_name) {
        let source = if existing.source == EditorSource::Local { "local" } else { "downloaded" };
        println!(
            "{}",
            Style::new().blue().apply_to(format!(
                "'{}' taken by {} ({})",
                display_name, source, existing.path.display()
            ))
        );
        let input: String = dialoguer::Input::new()
            .with_prompt("Choose another name")
            .default(display_name.to_string())
            .interact_text()?;
        Ok(input)
    } else {
        Ok(display_name.to_string())
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

    let filename = exe_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let (_parsed_version, display_name, is_mono_file) = github::parse_editor_name(&filename);

    let editor_name = resolve_editor_name(&display_name, config)?;

    let editor = EditorInfo {
        name: editor_name,
        path: exe_path,
        version: version_key.to_string(),
        is_mono: csharp || is_mono || is_mono_file,
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

    let filename = path
        .file_name()
        .context("Invalid path")?
        .to_string_lossy()
        .to_string();

    let (version, display_name, is_mono) = github::parse_editor_name(&filename);

    let needs_name = display_name == "Godot v." || display_name == "Godot vunknown" || version.is_empty() || version == "unknown";

    let editor_name = if needs_name {
        loop {
            let input: String = dialoguer::Input::new()
                .with_prompt("Editor name")
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
        }
    } else {
        display_name
    };

    let editor_version = if version.is_empty() || version == "unknown" {
        "local".to_string()
    } else {
        version
    };

    let editor = EditorInfo {
        name: editor_name,
        path: path.clone(),
        version: editor_version,
        is_mono,
        source: EditorSource::Local,
    };

    println!("Registered: {} ({})", editor.name, path.display());
    config.register_editor(editor.clone());
    config.save()?;
    Ok(Some(editor))
}
