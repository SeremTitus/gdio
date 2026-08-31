use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub fn parse_version_flavor(version: &str) -> (&str, &str) {
    let flavors = ["stable", "rc", "beta", "dev", "alpha"];

    // Strip mono suffix (e.g. "4.4.stable.mono" → "4.4.stable")
    let version = version.strip_suffix(".mono").unwrap_or(version);

    // Try dash separator first (e.g. "4.3-beta1")
    if let Some(idx) = version.rfind('-') {
        let potential_flavor = &version[idx + 1..];
        if flavors.iter().any(|s| potential_flavor.starts_with(s)) {
            return (&version[..idx], potential_flavor);
        }
    }

    // Try dot separator, searching backwards through all segments
    // (e.g. "4.8.dev.custom_build.7216a6290" → ("4.8", "dev"))
    let mut idx = version.len();
    while let Some(pos) = version[..idx].rfind('.') {
        let potential_flavor = &version[pos + 1..];
        if flavors.iter().any(|s| potential_flavor.starts_with(s)) {
            return (&version[..pos], potential_flavor);
        }
        idx = pos;
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
