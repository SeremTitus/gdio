use crate::config::{Config, EditorInfo};
use crate::project;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn chrono_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", secs)
}

pub fn register_opened_project(
    config: &mut Config,
    path: PathBuf,
    name: String,
    editor_version: &str,
) -> Result<()> {
    let now = chrono_now();
    let project_info = crate::config::ProjectInfo {
        path,
        name,
        bound_editor: Some(editor_version.to_string()),
        last_opened: Some(now),
    };
    config.register_project(&project_info);
    config.save()?;
    Ok(())
}

pub async fn resolve_editor(config: &mut Config, bound_editor: Option<&str>) -> Result<EditorInfo> {
    let mut editor = bound_editor
        .and_then(|v| config.find_editor_for_version(v))
        .cloned();

    if let Some(ref e) = editor
        && !e.path.exists()
    {
        println!("Editor binary not found: {}", e.path.display());
        editor = None;
    }

    match editor {
        Some(e) => Ok(e),
        None => {
            let mut options: Vec<String> =
                config.editors.values().map(|e| e.name.clone()).collect();
            options.push("[add editor]".to_string());

            let idx = dialoguer::FuzzySelect::new()
                .with_prompt("Select editor")
                .items(&options)
                .default(0)
                .interact()?;

            if options[idx] == "[add editor]" {
                let version: String = dialoguer::Input::new()
                    .with_prompt("Editor version (e.g. 4.7, 4.7-stable, or path)")
                    .interact_text()?;
                let csharp = dialoguer::Confirm::new()
                    .with_prompt("C# support?")
                    .default(false)
                    .interact()?;
                crate::commands::add::run(&version, None, csharp, config)
                    .await?
                    .context("Editor was not added")
            } else {
                let editors: Vec<_> = config.editors.values().cloned().collect();
                Ok(editors[idx].clone())
            }
        }
    }
}

pub fn cleanup_missing_projects(config: &mut Config) -> Result<bool> {
    let mut removed_count = 0u32;
    let paths_to_check: Vec<String> = config.projects.keys().cloned().collect();
    for path in paths_to_check {
        let project_file = Path::new(&path).join("project.godot");
        if !project_file.exists()
            && let Some(p) = config.remove_project(&path)
        {
            println!("Removed missing project: {} ({})", p.name, path);
            removed_count += 1;
        }
    }
    if removed_count > 0 {
        config.save()?;
        println!();
    }
    Ok(removed_count > 0)
}

pub fn compute_export_output_path(
    cwd: &Path,
    output_dir: &Path,
    project_snake: &str,
    preset_platform: &str,
    preset: &project::ExportPreset,
) -> PathBuf {
    if let Some(ref export_path) = preset.export_path {
        cwd.join(export_path)
    } else {
        let preset_snake = snake_case(&preset.name);
        match preset_platform {
            "web" => output_dir.join(&preset_snake).join("index.html"),
            "linux" => {
                let arch = preset.binary_format.as_deref().unwrap_or("x86_64");
                output_dir
                    .join(&preset_snake)
                    .join(format!("{}.{}", project_snake, arch))
            }
            _ => {
                let ext = match preset_platform {
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
            }
        }
    }
}

pub fn snake_case(s: &str) -> String {
    let raw: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let mut result = String::new();
    let mut prev_underscore = false;
    for c in raw.chars() {
        if c == '_' {
            if !prev_underscore && !result.is_empty() {
                result.push(c);
            }
            prev_underscore = true;
        } else {
            result.push(c);
            prev_underscore = false;
        }
    }
    result.trim_end_matches('_').to_string()
}
