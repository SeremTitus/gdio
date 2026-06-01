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

pub async fn fetch_release(version: &str, stage: &str) -> Result<GitHubRelease> {
    let tag = format!("{}-{}", version, stage);
    let url = format!("{}/tags/{}", GITHUB_API, tag);

    let client = reqwest::Client::builder()
        .user_agent("gdio")
        .build()?;

    let resp = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("Failed to fetch release info for {}", tag))?;

    if !resp.status().is_success() {
        anyhow::bail!("Release {} not found (HTTP {})", tag, resp.status());
    }

    let release: GitHubRelease = resp
        .json()
        .await
        .context("Failed to parse release info")?;
    Ok(release)
}

pub async fn fetch_release_auto(version: &str) -> Result<(GitHubRelease, String)> {
    if let Ok(release) = fetch_release(version, "stable").await {
        return Ok((release, "stable".to_string()));
    }

    let client = reqwest::Client::builder()
        .user_agent("gdio")
        .build()?;

    let url = format!("{}?per_page=100", GITHUB_API);
    let resp = client
        .get(&url)
        .send()
        .await
        .context("Failed to fetch releases list")?;

    if !resp.status().is_success() {
        anyhow::bail!("Failed to fetch releases list (HTTP {})", resp.status());
    }

    let releases: Vec<GitHubRelease> = resp
        .json()
        .await
        .context("Failed to parse releases list")?;

    let prefix = format!("{}-", version);
    let mut candidates: Vec<(String, &GitHubRelease)> = releases
        .iter()
        .filter(|r| r.tag_name.starts_with(&prefix))
        .map(|r| {
            let stage = &r.tag_name[prefix.len()..];
            (stage.to_string(), r)
        })
        .collect();

    if candidates.is_empty() {
        anyhow::bail!(
            "No release found for version {}. Check the version number.",
            version
        );
    }

    // Priority order: stable > rc (highest) > beta (highest) > dev (highest) > alpha (highest)
    let priority = |stage: &str| -> u32 {
        if stage == "stable" {
            100
        } else if let Some(n) = stage.strip_prefix("rc") {
            80 + n.parse::<u32>().unwrap_or(0)
        } else if let Some(n) = stage.strip_prefix("beta") {
            60 + n.parse::<u32>().unwrap_or(0)
        } else if let Some(n) = stage.strip_prefix("dev") {
            40 + n.parse::<u32>().unwrap_or(0)
        } else if let Some(n) = stage.strip_prefix("alpha") {
            20 + n.parse::<u32>().unwrap_or(0)
        } else {
            0
        }
    };

    candidates.sort_by_key(|b| std::cmp::Reverse(priority(&b.0)));

    let (stage, release) = candidates.remove(0);
    println!("Using release: {}-{}", version, stage);
    Ok((release.clone(), stage))
}