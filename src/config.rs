use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub fn parse_version_flavor(version: &str) -> (&str, &str) {
    let flavors = ["stable", "rc", "beta", "dev", "alpha"];

    // Try dash separator first (e.g. "4.3-beta1")
    if let Some(idx) = version.rfind('-') {
        let potential_flavor = &version[idx + 1..];
        if flavors.iter().any(|s| potential_flavor.starts_with(s)) {
            return (&version[..idx], potential_flavor);
        }
    }

    // Try dot separator (e.g. "4.3.beta1")
    if let Some(idx) = version.rfind('.') {
        let potential_flavor = &version[idx + 1..];
        if flavors.iter().any(|s| potential_flavor.starts_with(s)) {
            return (&version[..idx], potential_flavor);
        }
    }

    (version, "stable")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorInfo {
    pub name: String,
    pub path: PathBuf,
    pub version: String,
    pub is_mono: bool,
    pub source: EditorSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EditorSource {
    Downloaded,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub path: PathBuf,
    pub name: String,
    pub bound_editor: Option<String>,
    pub last_opened: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItchProjectConfig {
    pub game: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItchConfig {
    pub butler_path: String,
    pub projects: HashMap<String, ItchProjectConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub editors: HashMap<String, EditorInfo>,
    pub projects: HashMap<String, ProjectInfo>,
    pub recent_project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub itch: Option<ItchConfig>,
}

impl Config {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("gdio")
    }

    fn config_path() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    pub fn get_downloads_dir() -> PathBuf {
        Self::config_dir().join("downloads")
    }

    pub fn get_editors_dir() -> PathBuf {
        Self::config_dir().join("editors")
    }

    pub fn get_godot_data_dir() -> PathBuf {
        if cfg!(target_os = "windows") {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Godot")
        } else {
            dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Godot")
        }
    }

    pub fn get_godot_templates_dir() -> PathBuf {
        Self::get_godot_data_dir().join("export_templates")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if !path.exists() {
            return Ok(Config::default());
        }
        let data = fs::read_to_string(&path).context("Failed to read config file")?;
        let config: Config = serde_json::from_str(&data).context("Failed to parse config file")?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let dir = Self::config_dir();
        fs::create_dir_all(&dir).context("Failed to create config directory")?;
        let path = Self::config_path();
        let data = serde_json::to_string_pretty(self).context("Failed to serialize config")?;
        fs::write(&path, data).context("Failed to write config file")?;
        Ok(())
    }

    pub fn register_editor(&mut self, editor: EditorInfo) {
        let key = editor.version.clone();
        self.editors.insert(key, editor);
    }

    pub fn remove_editor(&mut self, version: &str) -> Option<EditorInfo> {
        self.editors.remove(version)
    }

    pub fn find_editor_for_version(&self, version: &str) -> Option<&EditorInfo> {
        if let Some(e) = self.editors.get(version) {
            return Some(e);
        }
        let requested = version.trim_start_matches('v');
        for (key, editor) in &self.editors {
            let stored = key.trim_start_matches('v');
            if stored.starts_with(requested) || requested.starts_with(stored) {
                return Some(editor);
            }
        }
        None
    }

    pub fn register_project(&mut self, project: ProjectInfo) {
        self.projects
            .insert(project.path.to_string_lossy().to_string(), project.clone());
        self.recent_project = Some(project.path.to_string_lossy().to_string());
    }

    pub fn update_project_editor(&mut self, project_path: &str, editor_version: &str) {
        if let Some(project) = self.projects.get_mut(project_path) {
            project.bound_editor = Some(editor_version.to_string());
        }
    }

    pub fn remove_project(&mut self, path: &str) -> Option<ProjectInfo> {
        let removed = self.projects.remove(path);
        if self.recent_project.as_deref() == Some(path) {
            self.recent_project = None;
        }
        removed
    }

    pub fn get_itch_config(&self) -> Option<&ItchConfig> {
        self.itch.as_ref()
    }

    pub fn get_itch_project(&self, project_path: &str) -> Option<&ItchProjectConfig> {
        self.itch
            .as_ref()
            .and_then(|itch| itch.projects.get(project_path))
    }

    pub fn set_itch_project(&mut self, project_path: &str, project_config: ItchProjectConfig) {
        let itch = self.itch.get_or_insert_with(|| ItchConfig {
            butler_path: "butler".to_string(),
            projects: HashMap::new(),
        });
        itch.projects
            .insert(project_path.to_string(), project_config);
    }

    pub fn get_or_default_itch(&mut self) -> &mut ItchConfig {
        self.itch.get_or_insert_with(|| ItchConfig {
            butler_path: "butler".to_string(),
            projects: HashMap::new(),
        })
    }
}

pub fn format_relative_time(timestamp: &str) -> String {
    let ts = match timestamp.parse::<u64>() {
        Ok(t) => t,
        Err(_) => return "never".to_string(),
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    if ts > now {
        return "just now".to_string();
    }
    let diff = now - ts;
    let _secs = diff % 60;
    let mins = diff / 60;
    let hours = mins / 60;
    let days = hours / 24;
    let weeks = days / 7;
    let months = days / 30;
    let years = days / 365;

    if diff < 60 {
        "just now".to_string()
    } else if mins < 60 {
        if mins == 1 { "1 min ago".to_string() } else { format!("{} min ago", mins) }
    } else if hours < 24 {
        let m = mins % 60;
        if hours == 1 && m == 0 {
            "1 h ago".to_string()
        } else if m == 0 {
            format!("{} h ago", hours)
        } else if hours == 1 {
            format!("1 h {} min ago", m)
        } else {
            format!("{} h {} min ago", hours, m)
        }
    } else if weeks < 4 {
        let d = days % 7;
        if weeks == 1 && d == 0 {
            "1 week ago".to_string()
        } else if d == 0 {
            format!("{} weeks ago", weeks)
        } else if weeks == 1 {
            format!("1 week {} d ago", d)
        } else {
            format!("{} weeks {} d ago", weeks, d)
        }
    } else if months < 12 {
        if months == 1 { "1 month ago".to_string() } else { format!("{} months ago", months) }
    } else {
        if years == 1 { "1 year ago".to_string() } else { format!("{} years ago", years) }
    }
}
