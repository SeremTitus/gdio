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

/// A third-party addon repository (beyond the default Godot Asset Store).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub name: String,
    pub url: String,
}

/// Metadata for a linked addon stored in `~/.config/gdio/addons/`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedAddonInfo {
    pub version: String,
    /// The folder name inside the addon ZIP (e.g. "gut" for bitwes/gut).
    pub folder_name: String,
    /// Project directory paths that reference this addon.
    #[serde(default)]
    pub projects: Vec<String>,
}

/// Metadata for an addon that should be installed in every project during sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalAddonEntry {
    /// Pinned version (set by `--select`). If `None`, the best compatible
    /// version is resolved per-project during sync.
    pub version: Option<String>,
    pub folder_name: String,
    /// The repository URL this addon was fetched from.
    pub repository: String,
    /// If true, store in global cache and symlink into each project.
    /// If false, copy directly into each project's addons/ directory.
    #[serde(default)]
    pub linked: bool,
}

/// Top-level addon configuration stored in the global gdio config.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AddonsConfig {
    /// Registered repositories (always includes the default godot-official-store).
    #[serde(default = "default_repositories")]
    pub repositories: Vec<Repository>,
    /// Addons stored in the global data folder with symlinks, keyed by `publisher/asset`.
    #[serde(default)]
    pub linked: HashMap<String, LinkedAddonInfo>,
    /// Addons that should be propagated to every project during sync.
    #[serde(default)]
    pub globals: HashMap<String, GlobalAddonEntry>,
    /// Per-addon list of project paths excluded from global sync.
    /// Key: `publisher/asset`, Value: list of project directory paths.
    #[serde(default)]
    pub globals_exclusions: HashMap<String, Vec<String>>,
    /// Sync counter — triggers orphan cleanup every 20 syncs.
    #[serde(default)]
    pub sync_count: u32,
}

/// Default repository: the official Godot Asset Store.
fn default_repositories() -> Vec<Repository> {
    vec![Repository {
        name: "godot-official-store".to_string(),
        url: "https://store.godotengine.org".to_string(),
    }]
}

/// Project-level addon tracking file (`.gdio`, TOML format).
///
/// Keys use `publisher/asset` identifiers (e.g. `[bitwes/gut]`). The custom
/// Serialize/Deserialize impls flatten `addons` to the root so the TOML file
/// has `[publisher/asset]` headers instead of `[addons."publisher/asset"]`.
/// TOML requires quoting keys with `/`, so `read_gdio` adds quotes before parsing
/// and `write_gdio` strips them after serialization.
#[derive(Debug, Clone, Default)]
pub struct GdioProject {
    /// Addon entries keyed by `publisher/asset` identifier.
    pub addons: HashMap<String, GdioAddonEntry>,
}

impl serde::Serialize for GdioProject {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.addons.len()))?;
        for (key, value) in &self.addons {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl<'de> serde::Deserialize<'de> for GdioProject {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let addons: HashMap<String, GdioAddonEntry> =
            HashMap::deserialize(deserializer)?;
        Ok(GdioProject { addons })
    }
}

/// A single addon entry in the `.gdio` project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GdioAddonEntry {
    pub version: String,
    /// Excluded from serialization if it's the default Godot Asset Store.
    #[serde(default, skip_serializing_if = "is_default_repository")]
    pub repository: String,
}


fn is_default_repository(url: &str) -> bool {
    url == "https://store.godotengine.org" || url.is_empty()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub editors: HashMap<String, EditorInfo>,
    pub projects: HashMap<String, ProjectInfo>,
    pub recent_project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub itch: Option<ItchConfig>,
    #[serde(default)]
    pub addons: AddonsConfig,
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

    pub fn get_global_addons_dir() -> PathBuf {
        Self::config_dir().join("addons")
    }

    pub fn get_addons_cache_dir() -> PathBuf {
        Self::get_downloads_dir()
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        let mut config: Config = if !path.exists() {
            Config::default()
        } else {
            let data = fs::read_to_string(&path).context("Failed to read config file")?;
            serde_json::from_str(&data).context("Failed to parse config file")?
        };
        // Ensure repositories list is never empty (serde default only applies when key is missing)
        if config.addons.repositories.is_empty() {
            config.addons.repositories = default_repositories();
        }
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
        let key = if editor.is_mono {
            format!("{}-csharp", editor.version)
        } else {
            editor.version.clone()
        };
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

    pub fn register_project(&mut self, project: &ProjectInfo) {
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
