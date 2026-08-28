pub mod addons;
pub mod editors;
pub mod itch;
pub mod projects;

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub use addons::{
    AddonsConfig, GdioAddonEntry, GdioProject, GlobalAddonEntry, LinkedAddonInfo, Repository,
    default_repositories,
};
pub use editors::{EditorInfo, EditorSource, parse_version_flavor};
pub use itch::{ItchConfig, ItchProjectConfig};
pub use projects::{ProjectInfo, format_relative_time};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Config {
    pub editors: HashMap<String, EditorInfo>,
    pub projects: HashMap<String, ProjectInfo>,
    pub recent_project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub itch: Option<ItchConfig>,
    #[serde(default)]
    pub addons: AddonsConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gdre_tools_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gdre_tools_last_checked: Option<u64>,
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

    pub fn get_gdre_tools_dir() -> PathBuf {
        Self::config_dir().join("gdre_tools")
    }

    pub fn get_butler_dir() -> PathBuf {
        Self::config_dir().join("butler")
    }

    pub fn get_butler_path() -> PathBuf {
        let dir = Self::get_butler_dir();
        if cfg!(target_os = "windows") {
            dir.join("butler.exe")
        } else {
            dir.join("butler")
        }
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
        let key = editor.key();
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

    pub fn get_or_default_itch(&mut self) -> &mut ItchConfig {
        self.itch.get_or_insert_with(|| ItchConfig {
            butler_version: None,
            butler_last_checked: None,
            projects: HashMap::new(),
        })
    }
}
