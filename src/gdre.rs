use crate::config::Config;
use anyhow::{Context, Result};
use futures_util::stream::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::path::{Path, PathBuf};

const GDRE_API: &str = "https://api.github.com/repos/GDRETools/gdsdecomp/releases/latest";
const CHECK_INTERVAL_SECONDS: u64 = 24 * 60 * 60; // 24 hours

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

fn platform_asset_pattern() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        ""
    }
}

fn github_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("gdio")
        .build()
        .context("Failed to build HTTP client")
}

async fn fetch_latest_release() -> Result<GhRelease> {
    let client = github_client()?;
    let resp = client
        .get(GDRE_API)
        .send()
        .await
        .context("Failed to fetch GDRE Tools latest release")?;

    if !resp.status().is_success() {
        anyhow::bail!(
            "Failed to fetch GDRE Tools releases (HTTP {})",
            resp.status()
        );
    }

    resp.json()
        .await
        .context("Failed to parse GDRE Tools release info")
}

fn find_platform_asset(release: &GhRelease) -> Option<&GhAsset> {
    let pattern = platform_asset_pattern();
    if pattern.is_empty() {
        return None;
    }

    release.assets.iter().find(|a| {
        let name = a.name.to_lowercase();
        name.ends_with(".zip") && name.contains(pattern) && !name.contains("android")
    })
}

async fn download_file(url: &str, dest: &Path) -> Result<()> {
    let client = github_client()?;
    let resp = client
        .get(url)
        .send()
        .await
        .context("Failed to start download")?;

    if !resp.status().is_success() {
        anyhow::bail!("Download failed: HTTP {}", resp.status());
    }

    let total_size = resp.content_length().unwrap_or(0);
    let file_name = dest
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{msg} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})",
            )
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=>-"),
    );
    if total_size > 0 {
        let size_mb = total_size as f64 / (1024.0 * 1024.0);
        pb.set_message(format!("Downloading {} ({:.1} MB)", file_name, size_mb));
    } else {
        pb.set_message(format!("Downloading {}", file_name));
    }

    let mut file = tokio::fs::File::create(dest)
        .await
        .context("Failed to create download file")?;
    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Failed to read download chunk")?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
        downloaded += chunk.len() as u64;
        pb.set_position(downloaded);
    }

    pb.finish_with_message(format!("Downloaded {}", file_name));
    println!();
    Ok(())
}

fn extract_zip(zip_path: &Path, dest_dir: &Path) -> Result<()> {
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file).context("Failed to open GDRE Tools zip")?;

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

    Ok(())
}

pub async fn ensure_gdre_tools(config: &mut Config) -> Result<PathBuf> {
    let tools_dir = Config::get_gdre_tools_dir();
    let exe_name = if cfg!(target_os = "windows") {
        "gdre_tools.exe"
    } else if cfg!(target_os = "macos") {
        "Godot RE Tools.app/Contents/MacOS/Godot RE Tools"
    } else {
        "gdre_tools"
    };
    let exe_path = tools_dir.join(exe_name);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let check_cached = config
        .gdre_tools
        .last_checked
        .map(|ts| now.saturating_sub(ts) < CHECK_INTERVAL_SECONDS)
        .unwrap_or(false);

    if exe_path.exists() && config.gdre_tools.version.is_some() && check_cached {
        return Ok(exe_path);
    }

    match fetch_latest_release().await {
        Ok(release) => {
            let latest_version = release.tag_name.trim_start_matches('v').to_string();

            let needs_update = match &config.gdre_tools.version {
                Some(current) => current != &latest_version || !exe_path.exists(),
                None => !exe_path.exists(),
            };

            if needs_update {
                let asset = find_platform_asset(&release)
                    .context("No GDRE Tools release found for this platform")?;

                println!("Updating GDRE Tools to v{}...\n", latest_version);

                let downloads_dir = Config::get_downloads_dir();
                std::fs::create_dir_all(&downloads_dir)?;
                let zip_path = downloads_dir.join(&asset.name);

                download_file(&asset.browser_download_url, &zip_path).await?;

                if tools_dir.exists() {
                    std::fs::remove_dir_all(&tools_dir)?;
                }
                std::fs::create_dir_all(&tools_dir)?;

                extract_zip(&zip_path, &tools_dir)?;
                let _ = std::fs::remove_file(&zip_path);

                #[cfg(target_os = "linux")]
                {
                    let target = tools_dir.join("gdre_tools");
                    if !target.exists() {
                        let arch_name = if cfg!(target_arch = "aarch64") {
                            "gdre_tools.aarch64"
                        } else {
                            "gdre_tools.x86_64"
                        };
                        let arch_path = tools_dir.join(arch_name);
                        if arch_path.exists() {
                            let _ = std::fs::rename(&arch_path, &target);
                        }
                    }
                }

                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    for entry in std::fs::read_dir(&tools_dir)
                        .into_iter()
                        .flatten()
                        .flatten()
                    {
                        let path = entry.path();
                        if path.is_file() {
                            let _ = std::fs::set_permissions(
                                &path,
                                std::fs::Permissions::from_mode(0o755),
                            );
                        }
                    }
                    if cfg!(target_os = "macos") {
                        let macos_dir = tools_dir.join("Godot RE Tools.app/Contents/MacOS");
                        if let Ok(entries) = std::fs::read_dir(&macos_dir) {
                            for entry in entries.flatten() {
                                if entry.path().is_file() {
                                    let _ = std::fs::set_permissions(
                                        entry.path(),
                                        std::fs::Permissions::from_mode(0o755),
                                    );
                                }
                            }
                        }
                    }
                }

                config.gdre_tools.version = Some(latest_version.clone());
                config.gdre_tools.last_checked = Some(now);
                config.save()?;

                println!("GDRE Tools v{} installed.\n", latest_version);
            } else {
                config.gdre_tools.last_checked = Some(now);
                let _ = config.save();
            }
        }
        Err(_) if exe_path.exists() => {
            eprintln!("Warning: Could not check for GDRE Tools updates, using installed version.");
            config.gdre_tools.last_checked = Some(now);
            let _ = config.save();
        }
        Err(e) => {
            anyhow::bail!(
                "Failed to check for GDRE Tools updates and no local copy exists: {}",
                e
            );
        }
    }

    Ok(exe_path)
}
