use std::path::Path;

pub fn parse_godot_version(project_path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(project_path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("config/features=")
            && let Some(start) = trimmed.find('"')
            && let Some(end) = trimmed[start + 1..].find('"')
        {
            let version = &trimmed[start + 1..start + 1 + end];
            return Some(version.to_string());
        }
    }
    None
}

pub fn parse_game_version(project_path: &Path) -> String {
    let content = match std::fs::read_to_string(project_path) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("config/version=")
            && let Some(start) = trimmed.find('"')
            && let Some(end) = trimmed[start + 1..].find('"')
        {
            let version = &trimmed[start + 1..start + 1 + end];
            if !version.is_empty() {
                return version.trim_start_matches('v').to_string();
            }
        }
    }
    String::new()
}

pub fn parse_project_name(project_path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(project_path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("config/name=")
            && let Some(start) = trimmed.find('"')
            && let Some(end) = trimmed[start + 1..].find('"')
        {
            let name = &trimmed[start + 1..start + 1 + end];
            return Some(name.to_string());
        }
    }
    None
}

pub struct ExportPreset {
    pub name: String,
    pub platform: String,
    pub export_path: Option<String>,
    pub binary_format: Option<String>,
}

pub fn parse_export_presets(project_path: &Path) -> Vec<ExportPreset> {
    let content = match std::fs::read_to_string(project_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let mut presets = Vec::new();
    let mut in_preset = false;
    let mut current_name = String::new();
    let mut current_platform = String::new();
    let mut current_export_path: Option<String> = None;
    let mut current_binary_format: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("[preset.") {
            if in_preset && !current_name.is_empty() {
                presets.push(ExportPreset {
                    name: current_name.clone(),
                    platform: current_platform.clone(),
                    export_path: current_export_path.clone(),
                    binary_format: current_binary_format.clone(),
                });
            }
            in_preset = true;
            current_name.clear();
            current_platform.clear();
            current_export_path = None;
            current_binary_format = None;
        } else if trimmed.starts_with('[') && in_preset {
            if !current_name.is_empty() {
                presets.push(ExportPreset {
                    name: current_name.clone(),
                    platform: current_platform.clone(),
                    export_path: current_export_path.clone(),
                    binary_format: current_binary_format.clone(),
                });
            }
            in_preset = false;
        } else if in_preset {
            if current_name.is_empty() && trimmed.starts_with("name=")
                && let Some(start) = trimmed.find('"')
                && let Some(end) = trimmed[start + 1..].find('"')
            {
                current_name = trimmed[start + 1..start + 1 + end].to_string();
            } else if current_platform.is_empty() && trimmed.starts_with("platform=")
                && let Some(start) = trimmed.find('"')
                && let Some(end) = trimmed[start + 1..].find('"')
            {
                current_platform = trimmed[start + 1..start + 1 + end].to_string();
            } else if current_export_path.is_none() && trimmed.starts_with("export_path=")
                && let Some(start) = trimmed.find('"')
                && let Some(end) = trimmed[start + 1..].find('"')
            {
                let path = trimmed[start + 1..start + 1 + end].to_string();
                if !path.is_empty() {
                    current_export_path = Some(path);
                }
            } else if trimmed.starts_with("binary_format/architecture=")
                && let Some(start) = trimmed.find('"')
                && let Some(end) = trimmed[start + 1..].find('"')
            {
                let arch = trimmed[start + 1..start + 1 + end].to_string();
                if !arch.is_empty() {
                    current_binary_format = Some(arch);
                }
            }
        }
    }
    if in_preset && !current_name.is_empty() {
        presets.push(ExportPreset {
            name: current_name,
            platform: current_platform,
            export_path: current_export_path,
            binary_format: current_binary_format,
        });
    }
    presets
}

pub fn godot_platform_to_gdio(godot_platform: &str) -> Option<&'static str> {
    match godot_platform {
        "Windows" | "Windows Desktop" => Some("windows"),
        "Linux" | "Linux/X11" => Some("linux"),
        "Web" | "HTML5" => Some("web"),
        "macOS" | "Mac OSX" => Some("macos"),
        "iOS" => Some("ios"),
        "Android" => Some("android"),
        _ => None,
    }
}