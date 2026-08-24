use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    let diff = std::time::Duration::from_secs(now - ts);
    let f = timeago::Formatter::new();
    f.convert(diff)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub path: PathBuf,
    pub name: String,
    pub bound_editor: Option<String>,
    pub last_opened: Option<String>,
}
