use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use futures_util::stream::StreamExt;

#[derive(Clone)]
pub struct Release {
    pub download_url: String,
    pub version: String,
    pub stable: bool,
    pub min_godot_version: Option<String>,
    pub max_godot_version: Option<String>,
}

/// Parse a version string like "4.3.1" or "v1.0.0-beta2" into components.
/// Strips leading `v` prefix. Missing components default to 0.
/// Pre-release suffixes can use `-` or `.` separator (e.g. "1.0.0-beta2" or "1.0.0.beta2").
/// Also captures the pre-release tier and number for comparison.
///
/// Pre-release tier ordering: dev < alpha < beta < rc < (stable)
/// Within the same tier: dev2 > dev1, beta3 > beta2, etc.
fn parse_version_components(version: &str) -> ([i32; 3], i32, i32) {
    let v = version.strip_prefix('v').unwrap_or(version);
    // Split off the pre-release suffix (after `-` or after a digit followed by `.` followed by non-digit)
    let (core, suffix) = find_prerelease(v);
    let parts: Vec<&str> = core.split('.').collect();
    let mut components = [0i32; 3];
    for (i, part) in parts.iter().take(3).enumerate() {
        components[i] = part.parse().unwrap_or(0);
    }
    // Parse pre-release tier and number: "beta3" → tier=2, number=3
    let (tier, number) = parse_pre_release(suffix);
    (components, tier, number)
}

/// Find where the pre-release suffix starts in a version string.
///
/// Handles both `-beta2` and `.beta2` separators.
/// The suffix starts when we see `-` or `.` followed by a non-digit letter.
fn find_prerelease(version: &str) -> (&str, &str) {
    let bytes = version.as_bytes();
    for i in 1..bytes.len() {
        if (bytes[i] == b'-' || bytes[i] == b'.') && i + 1 < bytes.len() && bytes[i + 1].is_ascii_alphabetic() {
            return (&version[..i], &version[i + 1..]);
        }
    }
    (version, "")
}

/// Parse a pre-release suffix into a tier number and suffix number.
///
/// Tier ordering: dev=0 < alpha=1 < beta=2 < rc=3 < stable=4
/// The suffix number is the trailing digits (e.g. "beta3" → 3).
fn parse_pre_release(suffix: &str) -> (i32, i32) {
    if suffix.is_empty() {
        return (4, 0); // stable
    }
    let (tier_name, number_str) = if let Some(pos) = suffix.find(|c: char| c.is_ascii_digit()) {
        (&suffix[..pos], &suffix[pos..])
    } else {
        (suffix, "0")
    };
    let tier = match tier_name {
        "dev" => 0,
        "alpha" | "a" => 1,
        "beta" | "b" => 2,
        "rc" => 3,
        _ => -1, // unknown tier, compare as string
    };
    let number: i32 = number_str.parse().unwrap_or(0);
    (tier, number)
}

/// Check if an engine version satisfies a version constraint.
///
/// Uses component-by-component comparison matching the Godot editor logic:
/// - `is_min = true`: returns true if `constraint <= engine` (engine is new enough)
/// - `is_min = false`: returns true if `constraint >= engine` (engine is old enough)
///
/// The first differing component determines the result.
pub fn is_version_compatible(engine_version: &str, constraint: &str, is_min: bool) -> bool {
    let (engine, _, _) = parse_version_components(engine_version);
    let (constraint_parts, _, _) = parse_version_components(constraint);

    for j in 0..3 {
        if engine[j] != constraint_parts[j] {
            if is_min {
                return constraint_parts[j] <= engine[j];
            } else {
                return constraint_parts[j] >= engine[j];
            }
        }
    }
    true
}

/// List all compatible releases sorted by version (highest first).
///
/// Filters by engine version compatibility and sorts for display.
pub fn list_compatible_releases<'a>(releases: &'a [Release], godot_version: &str) -> Vec<&'a Release> {
    let mut compatible: Vec<&Release> = releases
        .iter()
        .filter(|r| {
            if let Some(ref min) = r.min_godot_version
                && !is_version_compatible(godot_version, min, true) {
                    return false;
                }
            if let Some(ref max) = r.max_godot_version
                && !is_version_compatible(godot_version, max, false) {
                    return false;
                }
            true
        })
        .collect();

    compatible.sort_by_key(|b| std::cmp::Reverse(parse_version_key(&b.version)));
    compatible
}

/// Convert a version string into a tuple for lexicographic comparison.
/// Compares numeric components first, then pre-release tier, then suffix number.
fn parse_version_key(version: &str) -> (i32, i32, i32, i32, i32) {
    let (parts, tier, number) = parse_version_components(version);
    (parts[0], parts[1], parts[2], tier, number)
}

/// Fetch all releases for an asset from a repository's API.
///
/// Calls `GET {repo_url}/api/v1/releases/{publisher}/{asset}/` and parses
/// the JSON response into a list of `Release` entries.
pub async fn fetch_releases(
    client: &reqwest::Client,
    repo_url: &str,
    publisher: &str,
    asset: &str,
) -> Result<Vec<Release>> {
    // Build the API URL: https://store.godotengine.org/api/v1/releases/bitwes/gut/
    let url = format!(
        "{}/api/v1/releases/{}/{}/",
        repo_url.trim_end_matches('/'),
        publisher,
        asset
    );

    // Send GET request to the asset store API
    let resp = client
        .get(&url)
        .send()
        .await
        .context("Failed to fetch releases")?;

    // Check for HTTP errors (e.g. 404 if asset doesn't exist)
    if !resp.status().is_success() {
        if resp.status().as_u16() == 404 {
            anyhow::bail!("Addon '{}/{}' does not exist on the asset store.", publisher, asset);
        }
        anyhow::bail!(
            "Failed to fetch releases for {}/{}: HTTP {}",
            publisher,
            asset,
            resp.status()
        );
    }

    // Parse the JSON array of release objects
    let data: Vec<serde_json::Value> = resp.json().await.context("Failed to parse releases")?;

    // Convert JSON values into our Release struct, skipping entries missing required fields
    let releases = data
        .into_iter()
        .filter_map(|d| {
            Some(Release {
                download_url: d["download_url"].as_str()?.to_string(),
                version: d["version"].as_str()?.strip_prefix('v').unwrap_or(d["version"].as_str()?).to_string(),
                stable: d["stable"].as_bool().unwrap_or(false),
                min_godot_version: d["min_godot_version"].as_str().map(|s| s.to_string()),
                max_godot_version: d["max_godot_version"].as_str().map(|s| s.to_string()),
            })
        })
        .collect();

    Ok(releases)
}

/// Download a ZIP file from a URL to a local directory.
///
/// Displays a progress bar during download and returns the path to the saved file.
/// Retries up to 3 times with exponential backoff on transient failures.
pub async fn download_zip(
    client: &reqwest::Client,
    url: &str,
    dest_dir: &std::path::Path,
    filename: &str,
) -> Result<std::path::PathBuf> {
    std::fs::create_dir_all(dest_dir)?;
    let dest_path = dest_dir.join(filename);

    const MAX_RETRIES: u32 = 3;
    for attempt in 1..=MAX_RETRIES {
        match try_download(client, url, &dest_path, filename).await {
            Ok(()) => return Ok(dest_path),
            Err(e) => {
                // Check the full error chain for transient error indicators
                let chain = format!("{:#}", e);
                let is_transient = chain.contains("connection")
                    || chain.contains("timeout")
                    || chain.contains("TLS")
                    || chain.contains("tls")
                    || chain.contains("close_notify")
                    || chain.contains("chunked")
                    || chain.contains("decoding response body");

                if attempt < MAX_RETRIES && is_transient {
                    let delay = 2u64.pow(attempt); // 2s, 4s
                    eprintln!("  Download failed (attempt {}/{}). Retrying in {}s...", attempt, MAX_RETRIES, delay);
                    let _ = std::fs::remove_file(&dest_path);
                    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                } else {
                    return Err(e);
                }
            }
        }
    }
    unreachable!()
}

/// Attempt a single download. Called by download_zip with retry logic.
async fn try_download(
    client: &reqwest::Client,
    url: &str,
    dest_path: &std::path::Path,
    filename: &str,
) -> Result<()> {
    let resp = client
        .get(url)
        .send()
        .await
        .context("Failed to download addon")?;

    if !resp.status().is_success() {
        anyhow::bail!("Download failed: HTTP {}", resp.status());
    }

    let total_size = resp.content_length().unwrap_or(0);
    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("  {msg} [{elapsed_precise}] [{bar:30.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")
            .unwrap_or_else(|_| ProgressStyle::default_bar())
            .progress_chars("=>-"),
    );
    pb.set_message(format!("Downloading {}", filename));

    let mut file = std::fs::File::create(dest_path)?;
    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Failed to read chunk")?;
        std::io::Write::write_all(&mut file, &chunk)?;
        downloaded += chunk.len() as u64;
        pb.set_position(downloaded);
    }

    pb.finish_and_clear();
    Ok(())
}
