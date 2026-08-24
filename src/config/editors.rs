use serde::{Deserialize, Serialize};
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

impl EditorInfo {
    pub fn key(&self) -> String {
        if self.is_mono {
            format!("{}-csharp", self.version)
        } else {
            self.version.clone()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EditorSource {
    Downloaded,
    Local,
}
