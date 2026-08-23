use anyhow::{Context, Result};
use futures_util::stream::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::path::Path;
use tempfile::Builder;
use tokio::io::AsyncWriteExt;

use super::storage;

// Size of tail to read for Central Directory search (64KB)
const ZIP_CD_SEARCH_SIZE: u64 = 0x10000;
// Extra buffer for ZIP64 extended info in local header
const ZIP64_EXTRA_BUFFER: u64 = 256;

// Concurrency limit for parallel downloads
const DOWNLOAD_CONCURRENCY: usize = 5;

pub fn progress_style_file() -> ProgressStyle {
    ProgressStyle::default_bar()
        .template("  {msg} [{elapsed_precise}] [{bar:30.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("=>-")
}

pub fn progress_style_archive() -> ProgressStyle {
    ProgressStyle::default_bar()
        .template("  {msg} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("=>-")
}

pub fn progress_style_overall() -> ProgressStyle {
    ProgressStyle::default_bar()
        .template("  {msg} [{bar:30.green/black}] {pos}/{len} files")
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("=>-")
}

pub async fn fetch_mirror_url(
    client: &reqwest::Client,
    base_version: &str,
    flavor: &str,
) -> Result<String> {
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
                base_version,
                flavor
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

pub async fn download_files_concurrent(
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
                let result =
                    download_file_from_mirror(&client, &url, &filename, &dest, Some(&mp)).await;
                (filename, result)
            }
        })
        .collect();

    let results: Vec<_> = futures_util::stream::iter(pending)
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

pub async fn download_range(
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
        let total = end
            .checked_sub(start)
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

pub async fn download_file_from_mirror(
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
    let entries = storage::find_zip_entries(&tail_data, file_size)?;

    // Step 4: Find our target file
    let target_path = format!("templates/{}", filename);
    let entry = entries
        .iter()
        .find(|e| e.filename == target_path)
        .context(format!("File '{}' not found in archive", filename))?;

    // Step 5: Download the local file header + compressed data
    // Add buffer for ZIP64 extra data in local header (can differ from CD entry)
    let download_start = entry.offset;
    let download_end = (download_start
        + entry.local_header_size as u64
        + entry.compressed_size as u64
        + ZIP64_EXTRA_BUFFER)
        .min(file_size.saturating_sub(1));

    // Validate range
    if download_start > download_end {
        anyhow::bail!(
            "Invalid download range: start ({}) > end ({}), file_size={}, entry.offset={}",
            download_start,
            download_end,
            file_size,
            entry.offset
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
    }
    .await;

    // Guard will clean up progress bar on drop (including panic)
    result
}

pub async fn download_full_tpz(client: &reqwest::Client, url: &str, dest_dir: &Path) -> Result<()> {
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

    let temp_file = Builder::new()
        .suffix(".tpz")
        .tempfile_in(dest_dir)
        .context("Failed to create temp file")?;
    let temp_path = temp_file.path().to_path_buf();
    let mut tokio_file = tokio::fs::File::create(&temp_path)
        .await
        .context("Failed to open temp file for writing")?;

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

/// Download template files for a specific platform using range requests.
pub async fn download_template_files(
    client: &reqwest::Client,
    version: &str,
    platform: &str,
    dest: &Path,
) -> Result<()> {
    let (base_version, flavor) = crate::config::parse_version_flavor(version);

    let mirror_url = fetch_mirror_url(client, base_version, flavor).await?;
    println!("Using mirror: {}", mirror_url);

    let files = crate::github::platform_template_files(platform);
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
    let overall_pb = mp.add(ProgressBar::new(
        files.iter().filter(|f| !skipped.contains(*f)).count() as u64,
    ));
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
    )
    .await;

    overall_pb.finish_and_clear();
    mp.clear().ok();

    let mut failed = Vec::new();
    for (filename, result) in results {
        match result {
            Ok((name, size)) => println!("  ✓ {} ({} bytes)", name, size),
            Err(e) => {
                let red = console::Style::new().red();
                eprintln!("{}", red.apply_to(format!("  ✗ {}: {}", filename, e)));
                failed.push((filename, e));
            }
        }
    }

    if !failed.is_empty() {
        anyhow::bail!(
            "{} template download(s) failed: {}",
            failed.len(),
            failed
                .iter()
                .map(|(f, _)| f.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    Ok(())
}
