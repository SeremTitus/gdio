use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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