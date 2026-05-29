use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

const GITHUB_API: &str = "https://api.github.com/repos/godotengine/godot-builds/releases";

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubRelease {
    #[allow(dead_code)]
    pub tag_name: String,
    pub assets: Vec<GitHubAsset>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
}

pub fn platform_id() -> &'static str {
    if cfg!(target_os = "windows") {
        "win64"
    } else if cfg!(target_os = "linux") {
        "linux.x86_64"
    } else if cfg!(target_os = "macos") {
        "macos.universal"
    } else {
        "unknown"
    }
}

pub fn platform_dir_suffix() -> &'static str {
    if cfg!(target_os = "windows") {
        "win64"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "unknown"
    }
}

pub fn mono_dir_suffix() -> &'static str {
    if cfg!(target_os = "windows") {
        "mono_win64"
    } else if cfg!(target_os = "linux") {
        "mono_linux"
    } else if cfg!(target_os = "macos") {
        "mono_macos"
    } else {
        "unknown"
    }
}