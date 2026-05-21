use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

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
