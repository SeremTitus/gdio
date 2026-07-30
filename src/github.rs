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

pub fn find_editor_asset(release: &GitHubRelease, is_mono: bool) -> Option<&GitHubAsset> {
    let platform = platform_id();
    if platform == "unknown" {
        return None;
    }

    if cfg!(target_os = "windows") {
        let console = release.assets.iter().find(|a| {
            let name = &a.name;
            let matches_platform = name.contains(platform);
            let matches_mono = if is_mono {
                name.contains("mono")
            } else {
                !name.contains("mono")
            };
            let is_console = name.contains("console");
            let lower_name = name.to_lowercase();
            let is_editor = lower_name.ends_with(".zip")
                || (!lower_name.ends_with(".tpz") && !name.contains("export_templates"));
            matches_platform
                && matches_mono
                && is_console
                && is_editor
                && !name.contains("android")
                && !name.contains("ios")
        });
        if console.is_some() {
            return console;
        }
    }

    release.assets.iter().find(|a| {
        let name = &a.name;
        let matches_platform = name.contains(platform);
        let matches_mono = if is_mono {
            name.contains("mono")
        } else {
            !name.contains("mono")
        };
        let lower_name = name.to_lowercase();
        let is_editor = lower_name.ends_with(".zip")
            || (!lower_name.ends_with(".tpz") && !name.contains("export_templates"));
        matches_platform
            && matches_mono
            && is_editor
            && !name.contains("android")
            && !name.contains("ios")
    })
}

pub fn platform_template_files(platform: &str) -> Vec<&'static str> {
    match platform {
        "windows" => vec![
            "windows_debug_x86_32.exe",
            "windows_debug_x86_32_console.exe",
            "windows_release_x86_32.exe",
            "windows_release_x86_32_console.exe",
            "windows_debug_x86_64.exe",
            "windows_debug_x86_64_console.exe",
            "windows_release_x86_64.exe",
            "windows_release_x86_64_console.exe",
            "windows_debug_arm64.exe",
            "windows_debug_arm64_console.exe",
            "windows_release_arm64.exe",
            "windows_release_arm64_console.exe",
        ],
        "linux" => vec![
            "linux_debug.x86_32",
            "linux_release.x86_32",
            "linux_debug.x86_64",
            "linux_release.x86_64",
            "linux_debug.arm32",
            "linux_release.arm32",
            "linux_debug.arm64",
            "linux_release.arm64",
        ],
        "macos" => vec![
            "macos.zip",
        ],
        "web" => vec![
            "web_debug.zip",
            "web_release.zip",
            "web_dlink_debug.zip",
            "web_dlink_release.zip",
            "web_nothreads_debug.zip",
            "web_nothreads_release.zip",
            "web_dlink_nothreads_debug.zip",
            "web_dlink_nothreads_release.zip",
        ],
        "ios" => vec![
            "ios.zip",
        ],
        "android" => vec![
            "android_debug.apk",
            "android_release.apk",
            "android_source.zip",
        ],
        _ => vec![],
    }
}

pub async fn download_file(url: &str, dest: &Path) -> Result<()> {
    let client = reqwest::Client::builder()
        .user_agent("gdio")
        .build()?;

    let resp = client
        .get(url)
        .send()
        .await
        .context("Failed to start download")?;

    if !resp.status().is_success() {
        anyhow::bail!("Download failed: HTTP {}", resp.status());
    }

    let mut file = std::fs::File::create(dest).context("Failed to create download file")?;
    let content = resp.bytes().await.context("Failed to read download")?;
    std::io::Write::write_all(&mut file, &content)?;
    Ok(())
}

pub async fn download_and_extract_editor(
    version: &str,
    stage: &str,
    is_mono: bool,
    dest_dir: &Path,
) -> Result<(PathBuf, String)> {
    let release = fetch_release(version, stage).await?;
    let asset = find_editor_asset(&release, is_mono)
        .context("No matching editor asset found for this platform")?;

    println!("Downloading {}...", asset.name);

    let downloads_dir = crate::config::Config::get_downloads_dir();
    std::fs::create_dir_all(&downloads_dir)?;

    let zip_path = downloads_dir.join(&asset.name);
    download_file(&asset.browser_download_url, &zip_path).await?;

    let file = std::fs::File::open(&zip_path)?;
    let mut archive = zip::ZipArchive::new(file).context("Failed to open zip archive")?;

    std::fs::create_dir_all(dest_dir)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let outpath = dest_dir.join(entry.mangled_name());

        if entry.is_dir() {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut entry, &mut outfile)?;
        }
    }

    let exe = find_executable_in_dir(dest_dir)?;

    let _ = std::fs::remove_file(&zip_path);

    Ok((exe, stage.to_string()))
}

pub async fn download_and_extract_editor_auto(
    version: &str,
    is_mono: bool,
    dest_dir: &Path,
) -> Result<(PathBuf, String)> {
    let (release, stage) = fetch_release_auto(version).await?;
    let asset = find_editor_asset(&release, is_mono)
        .context("No matching editor asset found for this platform")?;

    println!("Downloading {}...", asset.name);

    let downloads_dir = crate::config::Config::get_downloads_dir();
    std::fs::create_dir_all(&downloads_dir)?;

    let zip_path = downloads_dir.join(&asset.name);
    download_file(&asset.browser_download_url, &zip_path).await?;

    let file = std::fs::File::open(&zip_path)?;
    let mut archive = zip::ZipArchive::new(file).context("Failed to open zip archive")?;

    std::fs::create_dir_all(dest_dir)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let outpath = dest_dir.join(entry.mangled_name());

        if entry.is_dir() {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut outfile = std::fs::File::create(&outpath)?;
            std::io::copy(&mut entry, &mut outfile)?;
        }
    }

    let exe = find_executable_in_dir(dest_dir)?;

    let _ = std::fs::remove_file(&zip_path);

    Ok((exe, stage))
}

fn find_executable_in_dir(dir: &Path) -> Result<PathBuf> {
    let mut console_exe = None;
    let mut regular_exe = None;

    if cfg!(target_os = "macos") {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() && path.extension().is_some_and(|e| e == "app") {
                let macos_bin = path.join("Contents").join("MacOS").join("Godot");
                if macos_bin.exists() {
                    return Ok(macos_bin);
                }
            }
        }
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let name = path.file_name().unwrap().to_string_lossy();
            let is_exe = if cfg!(target_os = "windows") {
                name.ends_with(".exe")
            } else {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::metadata(&path)
                        .map(|m| m.permissions().mode() & 0o111 != 0)
                        .unwrap_or(false)
                }
                #[cfg(not(unix))]
                {
                    false
                }
            };
            if is_exe && (name.to_lowercase().contains("godot") || name.starts_with("Godot")) {
                if name.contains("console") {
                    console_exe = Some(path);
                } else if regular_exe.is_none() {
                    regular_exe = Some(path);
                }
            }
        }
    }

    console_exe
        .or(regular_exe)
        .ok_or_else(|| anyhow::anyhow!("No Godot executable found in extracted directory"))
}

pub fn parse_editor_name(filename: &str) -> (String, String, bool) {
    let name = filename;
    let is_mono = name.to_lowercase().contains("mono");

    let version = if let Some(idx) = name.find('v') {
        let after_v = &name[idx + 1..];
        let end = after_v
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(after_v.len());
        after_v[..end].to_string()
    } else if let Some(idx) = name.find(|c: char| c.is_ascii_digit()) {
        let after = &name[idx..];
        let end = after
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(after.len());
        after[..end].to_string()
    } else {
        "unknown".to_string()
    };

    let display = format!("Godot v{}", version);

    (version, display, is_mono)
}
