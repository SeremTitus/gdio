use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItchProjectConfig {
    pub game: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItchConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub butler_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub butler_last_checked: Option<u64>,
    pub projects: HashMap<String, ItchProjectConfig>,
}

impl ItchConfig {
    pub fn get_project(&self, project_path: &str) -> Option<&ItchProjectConfig> {
        self.projects.get(project_path)
    }

    pub fn set_project(&mut self, project_path: &str, project_config: ItchProjectConfig) {
        self.projects
            .insert(project_path.to_string(), project_config);
    }
}
