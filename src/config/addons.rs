use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
pub fn default_repositories() -> Vec<Repository> {
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
        let addons: HashMap<String, GdioAddonEntry> = HashMap::deserialize(deserializer)?;
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
