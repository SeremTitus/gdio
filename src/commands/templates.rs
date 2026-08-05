use anyhow::{Context, Result};
use crate::config::{self, Config};
use crate::github;
use console::Style;
use std::path::Path;

// ZIP constants
const EOCD_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
const CD_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];

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
    offset: u32,
    compressed_size: u32,
    method: u16,
    local_header_size: u32,
}

fn find_zip_entries(data: &[u8], file_size: u64) -> Result<Vec<ZipEntry>> {
    // Find EOCD signature
    let eocd_pos = data
        .windows(4)
        .rposition(|w| w == EOCD_SIGNATURE)
        .context("Invalid ZIP: EOCD not found")?;

    let total_entries = decode_u16(data, eocd_pos + 10) as usize;
    let cd_start_offset = decode_u32(data, eocd_pos + 16) as usize;
    let buffer_start_abs = file_size as usize - data.len();
    let mut current_pos = cd_start_offset.saturating_sub(buffer_start_abs);

    let mut entries = Vec::new();

    for _ in 0..total_entries {
        if current_pos + 46 > data.len() {
            break;
        }
        if data[current_pos..current_pos + 4] != CD_SIGNATURE {
            break;
        }

        let method = decode_u16(data, current_pos + 10);
        let comp_size = decode_u32(data, current_pos + 20);
        let name_len = decode_u16(data, current_pos + 28) as usize;
        let extra_len = decode_u16(data, current_pos + 30) as usize;
        let comm_len = decode_u16(data, current_pos + 32) as usize;
        let local_offset = decode_u32(data, current_pos + 42);

        let name_bytes = &data[current_pos + 46..current_pos + 46 + name_len];
        let filename = String::from_utf8_lossy(name_bytes).to_string();

        let full_record_len = 46 + name_len + extra_len + comm_len;

        // Calculate local file header size (30 + name_len + extra_len)
        let local_extra_len = if local_offset as usize + 28 <= data.len() {
            decode_u16(data, local_offset as usize + 28) as u32
        } else {
            0
        };
        let local_header_size = 30 + name_len as u32 + local_extra_len;

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

async fn download_range(url: &str, start: u64, end: u64) -> Result<Vec<u8>> {
    let client = reqwest::Client::builder()
        .user_agent("gdio")
        .build()?;

    let range = format!("bytes={}-{}", start, end);
    let resp = client
        .get(url)
        .header("Range", &range)
        .send()
        .await
        .context("Failed to send range request")?;

    let bytes = resp.bytes().await.context("Failed to read response")?;
    Ok(bytes.to_vec())
}

async fn download_file_from_mirror(url: &str, filename: &str, dest: &Path) -> Result<()> {
    println!("  Fetching ZIP index...");

    // Step 1: HEAD request to get file size
    let client = reqwest::Client::builder()
        .user_agent("gdio")
        .build()?;

    let head_resp = client
        .head(url)
        .send()
        .await
        .context("Failed to get file size")?;

    let file_size: u64 = head_resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if file_size == 0 {
        anyhow::bail!("Could not determine file size");
    }

    // Step 2: Download last 64KB to read Central Directory
    let tail_start = file_size.saturating_sub(0x10000);
    let tail_data = download_range(url, tail_start, file_size - 1).await?;

    // Step 3: Find entries in Central Directory
    let entries = find_zip_entries(&tail_data, file_size)?;

    // Step 4: Find our target file
    let target_path = format!("templates/{}", filename);
    let entry = entries
        .iter()
        .find(|e| e.filename == target_path)
        .context(format!("File '{}' not found in archive", filename))?;

    println!(
        "  Found: {} ({} bytes compressed)",
        filename, entry.compressed_size
    );

    // Step 5: Download the local file header + compressed data
    // Add buffer for ZIP64 extra data in local header (can differ from CD entry)
    let download_start = entry.offset as u64;
    let download_end = download_start + entry.local_header_size as u64 + entry.compressed_size as u64 + 256;

    let fragment = download_range(url, download_start, download_end).await?;

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
    std::fs::write(dest, &decompressed)?;
    println!("  ✓ {} ({} bytes)", filename, decompressed.len());

    Ok(())
}

async fn download_full_tpz(url: &str, dest_dir: &Path) -> Result<()> {
    let client = reqwest::Client::builder()
        .user_agent("gdio")
        .build()?;

    println!("  Downloading full template archive...");

    let resp = client
        .get(url)
        .send()
        .await
        .context("Failed to download template archive")?;

    if !resp.status().is_success() {
        anyhow::bail!("Download failed: HTTP {}", resp.status());
    }

    let bytes = resp
        .bytes()
        .await
        .context("Failed to read template archive")?;

    // Write to temp file
    let downloads_dir = crate::config::Config::get_downloads_dir();
    std::fs::create_dir_all(&downloads_dir)?;
    let zip_path = downloads_dir.join("templates_download.tpz");
    std::fs::write(&zip_path, &bytes)?;

    // Extract zip
    let file = std::fs::File::open(&zip_path)?;
    let mut archive = zip::ZipArchive::new(file).context("Failed to open template archive")?;

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

    // Clean up
    let _ = std::fs::remove_file(&zip_path);

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
            if group.templates.len() == 1 {
                // Skip showing template name for single-template platforms
            } else {
                println!("    {}", template.name);
            }
        }
    }
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
    let existing = if godot_dir.exists() {
        let installed = get_installed_files(godot_dir.as_path())?;
        let mut exist = Vec::new();
        for platform in &platforms {
            let files = github::platform_template_files(platform);
            if files.iter().any(|f| installed.contains(*f)) {
                exist.push(platform.as_str());
            }
        }
        exist
    } else {
        Vec::new()
    };

    let to_download: Vec<&str> = platforms
        .iter()
        .filter(|p| !existing.contains(&p.as_str()))
        .map(|s| s.as_str())
        .collect();

    if to_download.is_empty() {
        println!("All requested templates for {} already exist.", version);
        return Ok(());
    }

    println!("Downloading templates for {} ({})", version, to_download.join(", "));

    std::fs::create_dir_all(&godot_dir)?;

    let rt = tokio::runtime::Runtime::new()?;

    if is_legacy {
        // Legacy (pre-4.x): download full .tpz archive
        let tpz_url = format!(
            "https://downloads.godotengine.org/?version={}&flavor={}&slug=export_templates.tpz&platform=templates",
            base_version, flavor
        );
        println!("Using URL: {}", tpz_url);
        rt.block_on(download_full_tpz(&tpz_url, &godot_dir))?;
    } else {
        // 4.x+: download individual files via mirror
        let mirror_url = rt.block_on(async {
            let client = reqwest::Client::builder()
                .user_agent("gdio")
                .build()?;

            let mirror_list_url = format!(
                "https://godotengine.org/mirrorlist/{}.{}.json",
                base_version,
                flavor
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

            let mirror_url = mirrors_data["mirrors"][0]["url"]
                .as_str()
                .context("No mirrors available")?
                .to_string();

            Ok::<_, anyhow::Error>(mirror_url)
        })?;

        println!("Using mirror: {}", mirror_url);

        rt.block_on(async {
            for platform in &to_download {
                let files = github::platform_template_files(platform);
                println!("\n{}:", platform);
                for filename in files {
                    let dest = godot_dir.join(filename);
                    if !dest.exists() {
                        if let Err(e) = download_file_from_mirror(&mirror_url, filename, &dest).await {
                            let red = Style::new().red();
                            eprintln!("{}", red.apply_to(format!("  ✗ {}: {}", filename, e)));
                        }
                    } else {
                        println!("  ✓ {} (exists)", filename);
                    }
                }
            }
            Ok::<(), anyhow::Error>(())
        })?;
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
    version: &str,
    platform: &str,
    dest: &Path,
) -> Result<()> {
    let (base_version, flavor) = config::parse_version_flavor(version);

    // Get mirror URL
    let client = reqwest::Client::builder()
        .user_agent("gdio")
        .build()?;

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

    let mirror_url = mirrors_data["mirrors"][0]["url"]
        .as_str()
        .context("No mirrors available")?
        .to_string();

    println!("Using mirror: {}", mirror_url);

    let files = github::platform_template_files(platform);
    println!("\n{}:", platform);
    for filename in files {
        if !dest.join(filename).exists() {
            if let Err(e) = download_file_from_mirror(&mirror_url, filename, &dest.join(filename)).await {
                let red = Style::new().red();
                eprintln!("{}", red.apply_to(format!("  ✗ {}: {}", filename, e)));
            }
        } else {
            println!("  ✓ {} (exists)", filename);
        }
    }

    Ok(())
}
