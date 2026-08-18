use anyhow::{Context, Result};
use crate::config::{self, Config};
use crate::github;
use console::Style;
use futures_util::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::path::Path;
use tempfile::Builder;
use tokio::io::AsyncWriteExt;

// ZIP constants
const EOCD_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
const CD_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];

// Size of tail to read for Central Directory search (64KB)
const ZIP_CD_SEARCH_SIZE: u64 = 0x10000;
// Extra buffer for ZIP64 extended info in local header
const ZIP64_EXTRA_BUFFER: u64 = 256;

// Concurrency limit for parallel downloads
const DOWNLOAD_CONCURRENCY: usize = 5;

// Helper to create progress style with fallback for invalid templates (should never fail with hardcoded templates)
fn progress_style_file() -> ProgressStyle {
    ProgressStyle::default_bar()
        .template("  {msg} [{elapsed_precise}] [{bar:30.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("=>-")
}

fn progress_style_archive() -> ProgressStyle {
    ProgressStyle::default_bar()
        .template("  {msg} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("=>-")
}

fn progress_style_overall() -> ProgressStyle {
    ProgressStyle::default_bar()
        .template("  {msg} [{bar:30.green/black}] {pos}/{len} files")
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("=>-")
}

async fn fetch_mirror_url(client: &reqwest::Client, base_version: &str, flavor: &str) -> Result<String> {
    let mirror_list_url = format!(
        "https://godotengine.org/mirrorlist/{}.{}.json",
        base_version, flavor
    );
    let resp = client
        .get(&mirror_list_url)
        .send()
        .await
        .context("Failed to fetch mirrors")?;

    if !resp.status().is_success() {
        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            anyhow::bail!(
                "Version {}-{} does not exist.\n\
                 For dev/beta/rc, use the full version number (e.g. 4.3-beta1, 4.3-dev5, 4.3-rc1).",
                base_version, flavor
            );
        }
        anyhow::bail!("Failed to fetch mirrors: HTTP {}", status);
    }

    let mirrors_data: serde_json::Value = resp.json().await?;

    // Filter mirrors: prefer proper HTTP directory mirrors over CGI endpoints
    let mirror_url = mirrors_data["mirrors"]
        .as_array()
        .context("No mirrors array")?
        .iter()
        .filter_map(|m| m["url"].as_str())
        .find(|url| !url.contains("downloads.godotengine.org/?version="))
        .or_else(|| {
            // Fallback to first mirror if no suitable one found
            mirrors_data["mirrors"][0]["url"].as_str()
        })
        .context("No mirrors available")?
        .to_string();

    Ok(mirror_url)
}

async fn download_files_concurrent(
    client: &reqwest::Client,
    mirror_url: &str,
    files: &[&str],
    dest_dir: &Path,
    mp: &MultiProgress,
    overall_pb: &ProgressBar,
    skipped: &std::collections::HashSet<&'static str>,
) -> Vec<(String, Result<(String, u64)>)> {
    let pending: Vec<_> = files
        .iter()
        .filter(|f| !skipped.contains(*f))
        .map(|filename| {
            let dest = dest_dir.join(filename);
            let url = mirror_url.to_string();
            let filename = (*filename).to_string();
            let mp = mp.clone();
            let client = client.clone();
            async move {
                let result = download_file_from_mirror(&client, &url, &filename, &dest, Some(&mp)).await;
                (filename, result)
            }
        })
        .collect();

    let results: Vec<_> = stream::iter(pending)
        .map(|f| {
            let overall_pb = overall_pb.clone();
            async move {
                let (filename, r) = f.await;
                overall_pb.inc(1);
                (filename, r)
            }
        })
        .buffer_unordered(DOWNLOAD_CONCURRENCY)
        .collect()
        .await;

    results
}

fn decode_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn decode_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

struct ZipEntry {
    filename: String,
    offset: u64,
    compressed_size: u64,
    method: u16,
    local_header_size: u32,
}

fn find_zip_entries(data: &[u8], file_size: u64) -> Result<Vec<ZipEntry>> {
    // Find EOCD signature
    let eocd_pos = data
        .windows(4)
        .rposition(|w| w == EOCD_SIGNATURE)
        .context("Invalid ZIP: EOCD not found")?;

    // Check for ZIP64 EOCD
    let mut cd_start_offset = decode_u32(data, eocd_pos + 16) as u64;
    let mut total_entries = decode_u16(data, eocd_pos + 10) as u64;

    if total_entries == 0xFFFF || cd_start_offset == 0xFFFFFFFF {
        // Look for ZIP64 EOCD locator
        if eocd_pos >= 20 {
            let zip64_locator_pos = eocd_pos - 20;
            if zip64_locator_pos + 20 <= data.len()
                && data[zip64_locator_pos..zip64_locator_pos + 4] == [0x50, 0x4b, 0x06, 0x07]
            {
                let zip64_eocd_offset = decode_u64(data, zip64_locator_pos + 8) as usize;
                let buffer_start_abs = file_size as usize - data.len();
                let zip64_eocd_pos = zip64_eocd_offset.saturating_sub(buffer_start_abs);

                if zip64_eocd_pos + 56 <= data.len()
                    && data[zip64_eocd_pos..zip64_eocd_pos + 4] == [0x50, 0x4b, 0x06, 0x06]
                {
                    total_entries = decode_u64(data, zip64_eocd_pos + 24);
                    cd_start_offset = decode_u64(data, zip64_eocd_pos + 48);
                }
            }
        }
    }

    let buffer_start_abs = file_size as usize - data.len();
    let mut current_pos = cd_start_offset.saturating_sub(buffer_start_abs as u64) as usize;

    let mut entries = Vec::new();

    for _ in 0..total_entries {
        if current_pos + 46 > data.len() {
            break;
        }
        if data[current_pos..current_pos + 4] != CD_SIGNATURE {
            break;
        }

        let method = decode_u16(data, current_pos + 10);
        let mut comp_size = decode_u32(data, current_pos + 20) as u64;
        let name_len = decode_u16(data, current_pos + 28) as usize;
        let extra_len = decode_u16(data, current_pos + 30) as usize;
        let comm_len = decode_u16(data, current_pos + 32) as usize;
        let mut local_offset = decode_u32(data, current_pos + 42) as u64;

        let full_record_len = 46 + name_len + extra_len + comm_len;
        if current_pos + full_record_len > data.len() {
            break;
        }

        let name_bytes = &data[current_pos + 46..current_pos + 46 + name_len];
        let filename = String::from_utf8_lossy(name_bytes).to_string();

        // Parse ZIP64 extra field if present
        if comp_size == 0xFFFFFFFF || local_offset == 0xFFFFFFFF {
            let extra_start = current_pos + 46 + name_len;
            let extra_end = extra_start + extra_len;
            if extra_end <= data.len() {
                parse_zip64_extra(&data[extra_start..extra_end], &mut comp_size, &mut local_offset);
            }
        }

        // Local file header minimum size: 30 + name_len (extra field read separately after download)
        let local_header_size = 30 + name_len as u32;

        entries.push(ZipEntry {
            filename,
            offset: local_offset,
            compressed_size: comp_size,
            method,
            local_header_size,
        });

        current_pos += full_record_len;
    }

    Ok(entries)
}

fn decode_u64(data: &[u8], offset: usize) -> u64 {
    if offset + 8 > data.len() {
        return 0;
    }
    u64::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
        data[offset + 4],
        data[offset + 5],
        data[offset + 6],
        data[offset + 7],
    ])
}

fn parse_zip64_extra(data: &[u8], comp_size: &mut u64, local_offset: &mut u64) {
    let mut pos = 0;
    while pos + 4 <= data.len() {
        let header_id = u16::from_le_bytes([data[pos], data[pos + 1]]);
        let data_size = u16::from_le_bytes([data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;

        if pos + data_size > data.len() {
            break;
        }

        if header_id == 0x0001 {
            // ZIP64 extended information extra field
            let mut field_pos = pos;
            if *comp_size == 0xFFFFFFFF && field_pos + 8 <= pos + data_size {
                *comp_size = decode_u64(data, field_pos);
                field_pos += 8;
            }
            if *local_offset == 0xFFFFFFFF && field_pos + 8 <= pos + data_size {
                *local_offset = decode_u64(data, field_pos);
            }
            break;
        }

        pos += data_size;
    }
}

async fn download_range(
    client: &reqwest::Client,
    url: &str,
    start: u64,
    end: u64,
    pb: Option<&ProgressBar>,
) -> Result<Vec<u8>> {
    let range = format!("bytes={}-{}", start, end);
    let resp = client
        .get(url)
        .header("Range", &range)
        .send()
        .await
        .context("Failed to send range request")?;

    if resp.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        anyhow::bail!("Expected 206 Partial Content, got {}", resp.status());
    }

    if let Some(pb) = pb {
        let total = end.checked_sub(start)
            .map(|d| d.saturating_add(1))
            .unwrap_or(u64::MAX);
        let mut stream = resp.bytes_stream();
        let mut buffer = Vec::with_capacity(total.min(100_000_000) as usize);
        let mut downloaded: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Failed to read chunk")?;
            buffer.extend_from_slice(&chunk);
            downloaded += chunk.len() as u64;
            pb.set_position(downloaded);
        }
        Ok(buffer)
    } else {
        let bytes = resp.bytes().await.context("Failed to read response")?;
        Ok(bytes.to_vec())
    }
}

async fn download_file_from_mirror(
    client: &reqwest::Client,
    url: &str,
    filename: &str,
    dest: &Path,
    mp: Option<&MultiProgress>,
) -> Result<(String, u64)> {
    // Step 1: HEAD request to get file size
    let head_resp = client
        .head(url)
        .send()
        .await
        .context("Failed to get file size")?;

    let mut file_size: u64 = head_resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    // Fallback: if HEAD doesn't return Content-Length, try GET with Range: bytes=0-0
    if file_size == 0 {
        let range_resp = client
            .get(url)
            .header("Range", "bytes=0-0")
            .send()
            .await
            .context("Failed to probe file size with range request")?;

        if range_resp.status() == reqwest::StatusCode::PARTIAL_CONTENT
            || range_resp.status().is_success()
        {
            file_size = range_resp
                .headers()
                .get("content-range")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split('/').nth(1))
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        }
    }

    if file_size == 0 {
        anyhow::bail!("Could not determine file size");
    }

    // Step 2: Download last 64KB to read Central Directory
    let tail_start = file_size.saturating_sub(ZIP_CD_SEARCH_SIZE);
    let tail_data = download_range(client, url, tail_start, file_size - 1, None)
        .await
        .context("Failed to download archive tail for Central Directory")?;

    // Step 3: Find entries in Central Directory
    let entries = find_zip_entries(&tail_data, file_size)?;

    // Step 4: Find our target file
    let target_path = format!("templates/{}", filename);
    let entry = entries
        .iter()
        .find(|e| e.filename == target_path)
        .context(format!("File '{}' not found in archive", filename))?;

    // Step 5: Download the local file header + compressed data
    // Add buffer for ZIP64 extra data in local header (can differ from CD entry)
    let download_start = entry.offset;
    let download_end = (download_start + entry.local_header_size as u64 + entry.compressed_size as u64 + ZIP64_EXTRA_BUFFER).min(file_size.saturating_sub(1));

    // Validate range
    if download_start > download_end {
        anyhow::bail!(
            "Invalid download range: start ({}) > end ({}), file_size={}, entry.offset={}",
            download_start, download_end, file_size, entry.offset
        );
    }

    let expected_size = download_end.saturating_sub(download_start) + 1;
    let pb = mp.map_or_else(
        || ProgressBar::new(expected_size),
        |m| m.add(ProgressBar::new(expected_size)),
    );
    pb.set_style(progress_style_file());
    pb.set_message(format!("Downloading {}", filename));

    // Use a guard to ensure progress bar cleanup on panic
    struct PbGuard<'a> {
        pb: ProgressBar,
        mp: Option<&'a MultiProgress>,
    }
    impl Drop for PbGuard<'_> {
        fn drop(&mut self) {
            self.pb.finish_and_clear();
            if let Some(m) = self.mp {
                m.remove(&self.pb);
            }
        }
    }
    let _guard = PbGuard { pb: pb.clone(), mp };

    let result = async {
        let fragment = download_range(client, url, download_start, download_end, Some(&pb))
        .await
        .context(format!("Failed to download file '{}'", filename))?;

        // Parse local file header to get actual offsets
        if fragment.len() < 30 {
            anyhow::bail!("Fragment too small for local file header");
        }

        let local_name_len = u16::from_le_bytes([fragment[26], fragment[27]]) as usize;
        let local_extra_len = u16::from_le_bytes([fragment[28], fragment[29]]) as usize;
        let actual_local_header_size = 30 + local_name_len + local_extra_len;

        let compressed_data = &fragment[actual_local_header_size..];

        // Step 6: Decompress the data

        let decompressed = if entry.method == 0 {
            // Stored (no compression)
            compressed_data.to_vec()
        } else if entry.method == 8 {
            // Deflate
            use flate2::read::DeflateDecoder;
            use std::io::Read;
            let mut decoder = DeflateDecoder::new(compressed_data);
            let mut decompressed = Vec::new();
            if let Err(e) = decoder.read_to_end(&mut decompressed) {
                anyhow::bail!("Failed to decompress data: {}", e);
            }
            decompressed
        } else {
            anyhow::bail!("Unsupported compression method: {}", entry.method);
        };

        // Step 7: Write the file
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let size = decompressed.len() as u64;
        std::fs::write(dest, &decompressed)?;

        Ok::<_, anyhow::Error>((filename.to_string(), size))
    }.await;

    // Guard will clean up progress bar on drop (including panic)
    result
}

async fn download_full_tpz(client: &reqwest::Client, url: &str, dest_dir: &Path) -> Result<()> {
    println!("  Downloading full template archive...");

    // Create directory before temp file
    std::fs::create_dir_all(dest_dir)?;

    let resp = client
        .get(url)
        .send()
        .await
        .context("Failed to download template archive")?;

    if !resp.status().is_success() {
        anyhow::bail!("Download failed: HTTP {}", resp.status());
    }

    let total_size = resp.content_length().unwrap_or(0);

    let pb = ProgressBar::new(total_size);
    pb.set_style(progress_style_archive());
    pb.set_message("Downloading");

    let temp_file = Builder::new().suffix(".tpz").tempfile_in(dest_dir).context("Failed to create temp file")?;
    let temp_path = temp_file.path().to_path_buf();
    let mut tokio_file = tokio::fs::File::create(&temp_path).await.context("Failed to open temp file for writing")?;

    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Failed to read chunk")?;
        tokio_file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        pb.set_position(downloaded);
    }

    tokio_file.flush().await?;
    drop(tokio_file);

    pb.finish_and_clear();

    // Reopen with sync API for zip extraction
    let file = std::fs::File::open(&temp_path)?;
    let mut archive = zip::ZipArchive::new(file).context("Failed to open template archive")?;

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

    // Clean up (tempfile will be dropped and cleaned up automatically)
    drop(temp_file);

    println!("  Template archive extracted successfully");
    Ok(())
}

pub fn detect_platforms(dir: &Path) -> Result<Vec<String>> {
    let installed = get_installed_files(dir)?;
    let mut platforms = Vec::new();

    if installed.iter().any(|f| f.starts_with("windows_")) {
        platforms.push("windows".to_string());
    }
    if installed.iter().any(|f| f.starts_with("linux_")) {
        platforms.push("linux".to_string());
    }
    if installed.iter().any(|f| f.starts_with("macos")) {
        platforms.push("macos".to_string());
    }
    if installed.iter().any(|f| f.starts_with("web_")) {
        platforms.push("web".to_string());
    }
    if installed.iter().any(|f| f.starts_with("ios")) {
        platforms.push("ios".to_string());
    }
    if installed.iter().any(|f| f.starts_with("android")) {
        platforms.push("android".to_string());
    }

    Ok(platforms)
}

pub fn run_list(_config: &Config) -> Result<()> {
    let godot_dir = Config::get_godot_templates_dir();

    if !godot_dir.exists() {
        println!("No export templates installed.");
        println!("Godot stores templates in: {}", godot_dir.display());
        println!("Use `gdio templates add <version>` to install.");
        return Ok(());
    }

    let mut found_any = false;

    for entry in std::fs::read_dir(&godot_dir)? {
        let entry = entry?;
        if entry.path().is_dir() {
            let version = entry.file_name().to_string_lossy().to_string();
            let template_dir = entry.path();
            let installed = get_installed_files(&template_dir)?;

            if installed.is_empty() {
                continue;
            }

            if !found_any {
                found_any = true;
            }

            println!("\n{}", version);
            print_template_tree(&template_dir, &installed);
        }
    }

    if !found_any {
        println!("No export templates found.");
        println!("Use `gdio templates add <version>` to install.");
    }

    Ok(())
}

struct TemplateGroup<'a> {
    platform: &'a str,
    templates: Vec<TemplateInfo<'a>>,
}

struct TemplateInfo<'a> {
    name: &'a str,
    files: Vec<&'a str>,
}

fn get_template_groups() -> Vec<TemplateGroup<'static>> {
    vec![
        TemplateGroup {
            platform: "Windows",
            templates: vec![
                TemplateInfo {
                    name: "Windows x86_32",
                    files: vec![
                        "windows_debug_x86_32.exe",
                        "windows_debug_x86_32_console.exe",
                        "windows_release_x86_32.exe",
                        "windows_release_x86_32_console.exe",
                    ],
                },
                TemplateInfo {
                    name: "Windows x86_64",
                    files: vec![
                        "windows_debug_x86_64.exe",
                        "windows_debug_x86_64_console.exe",
                        "windows_release_x86_64.exe",
                        "windows_release_x86_64_console.exe",
                    ],
                },
                TemplateInfo {
                    name: "Windows arm64",
                    files: vec![
                        "windows_debug_arm64.exe",
                        "windows_debug_arm64_console.exe",
                        "windows_release_arm64.exe",
                        "windows_release_arm64_console.exe",
                    ],
                },
            ],
        },
        TemplateGroup {
            platform: "Linux",
            templates: vec![
                TemplateInfo {
                    name: "Linux x86_32",
                    files: vec!["linux_debug.x86_32", "linux_release.x86_32"],
                },
                TemplateInfo {
                    name: "Linux x86_64",
                    files: vec!["linux_debug.x86_64", "linux_release.x86_64"],
                },
                TemplateInfo {
                    name: "Linux arm32",
                    files: vec!["linux_debug.arm32", "linux_release.arm32"],
                },
                TemplateInfo {
                    name: "Linux arm64",
                    files: vec!["linux_debug.arm64", "linux_release.arm64"],
                },
            ],
        },
        TemplateGroup {
            platform: "macOS",
            templates: vec![TemplateInfo {
                name: "macOS",
                files: vec!["macos.zip"],
            }],
        },
        TemplateGroup {
            platform: "Web",
            templates: vec![
                TemplateInfo {
                    name: "Web",
                    files: vec!["web_debug.zip", "web_release.zip"],
                },
                TemplateInfo {
                    name: "Web with Extensions",
                    files: vec!["web_dlink_debug.zip", "web_dlink_release.zip"],
                },
                TemplateInfo {
                    name: "Web Single-Threaded",
                    files: vec!["web_nothreads_debug.zip", "web_nothreads_release.zip"],
                },
                TemplateInfo {
                    name: "Web with Extensions Single-Threaded",
                    files: vec![
                        "web_dlink_nothreads_debug.zip",
                        "web_dlink_nothreads_release.zip",
                    ],
                },
            ],
        },
        TemplateGroup {
            platform: "Android",
            templates: vec![TemplateInfo {
                name: "Android",
                files: vec![
                    "android_debug.apk",
                    "android_release.apk",
                    "android_source.zip",
                ],
            }],
        },
        TemplateGroup {
            platform: "iOS",
            templates: vec![TemplateInfo {
                name: "iOS",
                files: vec!["ios.zip"],
            }],
        },
    ]
}

fn get_installed_files(dir: &Path) -> Result<std::collections::HashSet<String>> {
    let mut files = std::collections::HashSet::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.path().is_file() {
            let name = entry.file_name().to_string_lossy().to_string();
            files.insert(name);
        }
    }
    Ok(files)
}

fn print_template_tree(_dir: &Path, installed: &std::collections::HashSet<String>) {
    let groups = get_template_groups();

    for group in &groups {
        let mut has_any = false;

        // Check if this platform has any templates
        for template in &group.templates {
            if template.files.iter().any(|f| installed.contains(*f)) {
                has_any = true;
                break;
            }
        }

        if !has_any {
            continue;
        }

        println!("  {}", group.platform);

        for template in &group.templates {
            let present: Vec<&str> = template
                .files
                .iter()
                .filter(|f| installed.contains(**f))
                .copied()
                .collect();

            if present.is_empty() {
                continue;
            }

            // For single-template platforms (macOS, Android, iOS), skip template level
            if group.templates.len() > 1 {
                println!("    {}", template.name);
            }
        }
    }
}

async fn download_modern_templates(
    client: &reqwest::Client,
    base_version: &str,
    flavor: &str,
    to_download: &[&str],
    godot_dir: &Path,
) -> Result<()> {
    let mirror_url = fetch_mirror_url(client, base_version, flavor).await?;
    println!("Using mirror: {}", mirror_url);

    let mp = MultiProgress::new();

    #[derive(Clone)]
    struct FileTask {
        platform: String,
        filename: String,
        skipped: bool,
    }

    let mut all_tasks = Vec::new();

    for platform in to_download {
        let files = github::platform_template_files(platform);
        for filename in files {
            let skipped = godot_dir.join(filename).exists();
            all_tasks.push(FileTask {
                platform: platform.to_string(),
                filename: (*filename).to_string(),
                skipped,
            });
        }
    }

    let mut seen_platform = std::collections::HashSet::new();
    for task in &all_tasks {
        if task.skipped {
            if !seen_platform.contains(&task.platform) {
                println!("\n{}:", task.platform);
                seen_platform.insert(task.platform.clone());
            }
            println!("  ✓ {} (exists)", task.filename);
        }
    }

    let to_download_tasks: Vec<_> = all_tasks.into_iter().filter(|t| !t.skipped).collect();
    let overall_pb = mp.add(ProgressBar::new(to_download_tasks.len() as u64));
    overall_pb.set_style(progress_style_overall());
    overall_pb.set_message("Downloading templates");

    let files: Vec<&str> = to_download_tasks.iter().map(|t| t.filename.as_ref()).collect();
    let results = download_files_concurrent(
        client,
        &mirror_url,
        &files,
        godot_dir,
        &mp,
        &overall_pb,
        &std::collections::HashSet::new(),
    ).await;

    let mut task_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for task in &to_download_tasks {
        task_map.insert(task.filename.clone(), task.platform.clone());
    }

    let mut printed_platform = std::collections::HashSet::new();
    let mut failed = Vec::new();
    for (filename, result) in results {
        let platform = task_map.get(&filename).cloned().unwrap_or_default();
        if !printed_platform.contains(&platform) && !platform.is_empty() {
            println!("\n{}:", platform);
            printed_platform.insert(platform.clone());
        }
        match result {
            Ok((name, size)) => println!("  ✓ {} ({} bytes)", name, size),
            Err(e) => {
                let red = Style::new().red();
                eprintln!("{}", red.apply_to(format!("  ✗ {}: {}", filename, e)));
                failed.push((filename, e));
            }
        }
    }

    overall_pb.finish_and_clear();
    mp.clear().ok();

    if !failed.is_empty() {
        anyhow::bail!(
            "{} template download(s) failed: {}",
            failed.len(),
            failed.iter().map(|(f, _)| f.as_str()).collect::<Vec<_>>().join(", ")
        );
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run_add(
    version: &str,
    windows: bool,
    linux: bool,
    web: bool,
    macos: bool,
    ios: bool,
    android: bool,
    _config: &mut Config,
) -> Result<()> {
    let (base_version, flavor) = config::parse_version_flavor(version);

    // Check if this is a pre-4.x version (full .tpz download)
    let is_legacy = base_version
        .split('.')
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .is_some_and(|major| major < 4);

    let has_flag = windows || linux || web || macos || ios || android;
    let mut platforms = Vec::new();
    if !has_flag {
        platforms = vec![
            "windows".to_string(),
            "linux".to_string(),
            "web".to_string(),
            "macos".to_string(),
            "ios".to_string(),
            "android".to_string(),
        ];
    } else {
        if windows {
            platforms.push("windows".to_string());
        }
        if linux {
            platforms.push("linux".to_string());
        }
        if web {
            platforms.push("web".to_string());
        }
        if macos {
            platforms.push("macos".to_string());
        }
        if ios {
            platforms.push("ios".to_string());
        }
        if android {
            platforms.push("android".to_string());
        }
    }

    let godot_dir = Config::get_godot_templates_dir().join(format!("{}.{}", base_version, flavor));

    // Check which platforms already exist
    let existing: Vec<&str> = if godot_dir.exists() {
        let installed = get_installed_files(godot_dir.as_path())?;
        let mut exist = Vec::new();
        for platform in &platforms {
            let files = github::platform_template_files(platform);
            if files.iter().any(|f| installed.contains(*f)) {
                exist.push(platform.as_ref());
            }
        }
        exist
    } else {
        Vec::new()
    };

    let to_download: Vec<&str> = platforms
        .iter()
        .filter(|p| !existing.contains(&p.as_ref()))
        .map(|s| s.as_ref())
        .collect();

    if to_download.is_empty() {
        println!("All requested templates for {} already exist.", version);
        return Ok(());
    }

    println!("Downloading templates for {} ({})", version, to_download.join(", "));

    std::fs::create_dir_all(&godot_dir)?;

    let rt = tokio::runtime::Runtime::new()?;

    let client = reqwest::Client::builder()
        .user_agent("gdio")
        .build()?;

    if is_legacy {
        // Legacy (pre-4.x): download full .tpz archive
        let tpz_url = format!(
            "https://downloads.godotengine.org/?version={}&flavor={}&slug=export_templates.tpz&platform=templates",
            base_version, flavor
        );
        println!("Using URL: {}", tpz_url);
        rt.block_on(download_full_tpz(&client, &tpz_url, &godot_dir))?;
    } else {
        // 4.x+: download individual files via mirror
        rt.block_on(download_modern_templates(
            &client,
            base_version,
            flavor,
            &to_download,
            &godot_dir,
        ))?;
    }

    println!("\nTemplates installed to: {}", godot_dir.display());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run_remove(
    version: &str,
    windows: bool,
    linux: bool,
    web: bool,
    macos: bool,
    ios: bool,
    android: bool,
    _config: &mut Config,
) -> Result<()> {
    let (base_version, flavor) = config::parse_version_flavor(version);
    let godot_dir = Config::get_godot_templates_dir().join(format!("{}.{}", base_version, flavor));

    if !godot_dir.exists() {
        anyhow::bail!("No templates found for version: {}", version);
    }

    let has_flag = windows || linux || web || macos || ios || android;

    if !has_flag {
        std::fs::remove_dir_all(&godot_dir)?;
        println!("Removed all templates for Godot {}", version);
    } else {
        let platforms_to_remove: Vec<&str> = [
            ("windows", windows),
            ("linux", linux),
            ("web", web),
            ("macos", macos),
            ("ios", ios),
            ("android", android),
        ]
        .iter()
        .filter(|(_, selected)| *selected)
        .map(|(name, _)| *name)
        .collect();

        for entry in std::fs::read_dir(&godot_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();

            let should_remove = platforms_to_remove.iter().any(|p| match *p {
                "windows" => name.starts_with("windows_"),
                "linux" => name.starts_with("linux_"),
                "macos" => name.starts_with("macos"),
                "web" => name.starts_with("web_"),
                "ios" => name.starts_with("ios"),
                "android" => name.starts_with("android"),
                _ => false,
            });

            if should_remove {
                if entry.path().is_dir() {
                    std::fs::remove_dir_all(entry.path())?;
                } else {
                    std::fs::remove_file(entry.path())?;
                }
                println!("Removed: {}", name);
            }
        }

        if std::fs::read_dir(&godot_dir)?.next().is_none() {
            std::fs::remove_dir(&godot_dir)?;
        }

        println!("Removed {} templates for Godot {}", platforms_to_remove.join(", "), version);
    }

    Ok(())
}

pub async fn download_template_files(
    client: &reqwest::Client,
    version: &str,
    platform: &str,
    dest: &Path,
) -> Result<()> {
    let (base_version, flavor) = config::parse_version_flavor(version);

    let mirror_url = fetch_mirror_url(client, base_version, flavor).await?;
    println!("Using mirror: {}", mirror_url);

    let files = github::platform_template_files(platform);
    println!("\n{}:", platform);

    let skipped: std::collections::HashSet<&'static str> = files
        .iter()
        .filter(|f| dest.join(f).exists())
        .copied()
        .collect();

    for filename in &skipped {
        println!("  ✓ {} (exists)", filename);
    }

    let mp = MultiProgress::new();
    let overall_pb = mp.add(ProgressBar::new(files.iter().filter(|f| !skipped.contains(*f)).count() as u64));
    overall_pb.set_style(progress_style_overall());
    overall_pb.set_message(format!("{platform} templates"));

    let results = download_files_concurrent(
        client,
        &mirror_url,
        &files,
        dest,
        &mp,
        &overall_pb,
        &skipped,
    ).await;

    overall_pb.finish_and_clear();
    mp.clear().ok();

    let mut failed = Vec::new();
    for (filename, result) in results {
        match result {
            Ok((name, size)) => println!("  ✓ {} ({} bytes)", name, size),
            Err(e) => {
                let red = Style::new().red();
                eprintln!("{}", red.apply_to(format!("  ✗ {}: {}", filename, e)));
                failed.push((filename, e));
            }
        }
    }

    if !failed.is_empty() {
        anyhow::bail!(
            "{} template download(s) failed: {}",
            failed.len(),
            failed.iter().map(|(f, _)| f.as_str()).collect::<Vec<_>>().join(", ")
        );
    }

    Ok(())
}
