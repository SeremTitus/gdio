use crate::config::{self, Config};
use crate::github;
use crate::platform::PlatformFlags;
use anyhow::Result;
use console::Style;
use indicatif::{MultiProgress, ProgressBar};
use std::path::Path;

use super::{api, list};

async fn download_modern_templates(
    client: &reqwest::Client,
    base_version: &str,
    flavor: &str,
    csharp: bool,
    to_download: &[&str],
    godot_dir: &Path,
) -> Result<()> {
    let mirror_url = api::fetch_mirror_url(client, base_version, flavor, csharp).await?;
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
    overall_pb.set_style(api::progress_style_overall());
    overall_pb.set_message("Downloading templates");

    let files: Vec<&str> = to_download_tasks
        .iter()
        .map(|t| t.filename.as_ref())
        .collect();
    let results = api::download_files_concurrent(
        client,
        &mirror_url,
        &files,
        godot_dir,
        &mp,
        &overall_pb,
        &std::collections::HashSet::new(),
    )
    .await;

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
            failed
                .iter()
                .map(|(f, _)| f.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    Ok(())
}

pub async fn run(
    version: &str,
    platform: &PlatformFlags,
    csharp: bool,
    _config: &mut Config,
) -> Result<()> {
    let (base_version, flavor) = config::parse_version_flavor(version);

    // Check if this is a pre-4.x version (full .tpz download)
    let is_legacy = base_version
        .split('.')
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .is_some_and(|major| major < 4);

    let platforms = if !platform.any() {
        vec![
            "windows".to_string(),
            "linux".to_string(),
            "web".to_string(),
            "macos".to_string(),
            "ios".to_string(),
            "android".to_string(),
            "visionos".to_string(),
        ]
    } else {
        platform.to_platforms()
    };

    let godot_dir = Config::get_godot_templates_dir().join(if csharp {
        format!("{}.{}.mono", base_version, flavor)
    } else {
        format!("{}.{}", base_version, flavor)
    });

    // Check which platforms already exist
    let existing: Vec<&str> = if godot_dir.exists() {
        let installed = list::get_installed_files(godot_dir.as_path())?;
        let mut exist = Vec::new();
        for platform in &platforms {
            let files = github::platform_template_files(platform);
            if files.iter().all(|f| installed.contains(*f)) {
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

    println!(
        "Downloading templates for {} ({})",
        version,
        to_download.join(", ")
    );

    std::fs::create_dir_all(&godot_dir)?;

    let client = reqwest::Client::builder().user_agent("gdio").build()?;

    if is_legacy {
        // Legacy (pre-4.x): download full .tpz archive
        let slug = if csharp {
            "mono_export_templates.tpz"
        } else {
            "export_templates.tpz"
        };
        let tpz_url = format!(
            "https://downloads.godotengine.org/?version={}&flavor={}&slug={}&platform=templates",
            base_version, flavor, slug
        );
        println!("Using URL: {}", tpz_url);
        api::download_full_tpz(&client, &tpz_url, &godot_dir).await?;
    } else {
        // 4.x+: download individual files via mirror
        download_modern_templates(
            &client,
            base_version,
            flavor,
            csharp,
            &to_download,
            &godot_dir,
        )
        .await?;
    }

    println!("\nTemplates installed to: {}", godot_dir.display());
    Ok(())
}
