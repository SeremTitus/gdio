use crate::config::Config;
use crate::godot;
use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::io::IsTerminal;
use std::io::Read;
use std::process::{Command, Stdio};

pub async fn run(
    url: &str,
    dir: Option<&str>,
    depth: Option<u32>,
    config: &mut Config,
) -> Result<()> {
    let project_dir = dir
        .map(|d| d.to_string())
        .unwrap_or_else(|| dir_from_url(url));

    if project_dir.is_empty() {
        anyhow::bail!("Could not determine directory name from URL. Provide a directory name.");
    }

    let cwd = std::env::current_dir().context("Failed to get current directory")?;

    if cwd.join("project.godot").exists() {
        if std::io::stdin().is_terminal() {
            let proceed = dialoguer::Confirm::new()
                .with_prompt("Current directory is already a Godot project. Clone here anyway?")
                .default(false)
                .interact()?;
            if !proceed {
                anyhow::bail!("Aborted.");
            }
        }
    }

    let target = cwd.join(&project_dir);

    if target.exists() {
        anyhow::bail!(
            "Directory '{}' already exists. \
             Choose a different name or remove it first.",
            project_dir
        );
    }

    if let Some(d) = depth {
        println!("Cloning {} depth={} into {}...", url, d, project_dir);
    } else {
        println!("Cloning {} into {}...", url, project_dir);
    }

    let mut args = vec![
        "clone".to_string(),
        "--progress".to_string(),
        "--recurse-submodules".to_string(),
        url.to_string(),
        project_dir.clone(),
    ];
    if let Some(d) = depth {
        args.insert(1, "--depth".to_string());
        args.insert(2, d.to_string());
    }

    let pb = ProgressBar::new(0);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len}")
            .unwrap()
            .progress_chars("=>-"),
    );
    pb.set_message("Cloning");

    let mut child = Command::new("git")
        .args(&args)
        .current_dir(&cwd)
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .context("Failed to run git. Is git installed?")?;

    if let Some(mut stderr) = child.stderr.take() {
        let mut phase = String::new();
        let mut total: u64 = 0;
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];

        loop {
            match stderr.read(&mut byte) {
                Ok(0) => break,
                Ok(_) => {
                    if byte[0] == b'\r' || byte[0] == b'\n' {
                        if !buf.is_empty() {
                            let line = String::from_utf8_lossy(&buf).trim().to_string();
                            let line = strip_sideband_prefix(&line);
                            if let Some(parsed) = parse_progress_line(&line) {
                                let (new_phase, count, done) = parsed;

                                if new_phase != phase {
                                    phase = new_phase;
                                    pb.set_message(format!("{}", phase));
                                }

                                if count > 0 && count != total {
                                    total = count;
                                    pb.set_length(total);
                                }

                                if total > 0 {
                                    pb.set_position(done);
                                }
                            }
                            buf.clear();
                        }
                    } else {
                        buf.push(byte[0]);
                    }
                }
                Err(_) => break,
            }
        }
    }

    let status = child.wait().context("Failed to wait for git clone")?;
    pb.finish_and_clear();

    if !status.success() {
        anyhow::bail!("git clone failed with exit code: {:?}", status.code());
    }

    let project_file = target.join("project.godot");
    if !project_file.exists() {
        anyhow::bail!(
            "Cloned repository does not contain a project.godot file. \
             This does not appear to be a Godot project."
        );
    }

    let original_dir = std::env::current_dir()?;
    std::env::set_current_dir(&target)?;
    let result = open_project(&project_dir, config).await;
    let _ = std::env::set_current_dir(original_dir);
    result
}

fn strip_sideband_prefix(line: &str) -> String {
    let bytes = line.as_bytes();
    if !bytes.is_empty() && (bytes[0] == 0x01 || bytes[0] == 0x02 || bytes[0] == 0x03) {
        return line[1..].to_string();
    }
    line.to_string()
}

fn parse_progress_line(line: &str) -> Option<(String, u64, u64)> {
    let phases = [
        ("remote: Counting objects", "Counting objects"),
        ("remote: Compressing objects", "Compressing objects"),
        ("Receiving objects", "Receiving objects"),
        ("Resolving deltas", "Resolving deltas"),
        ("Updating files", "Updating files"),
    ];

    for (prefix, label) in &phases {
        if let Some(rest) = line.strip_prefix(prefix) {
            let rest = rest.trim_start().trim_start_matches(':').trim_start();
            if let Some(pct_end) = rest.find('%') {
                let pct_str = rest[..pct_end].trim();
                if let Ok(pct) = pct_str.parse::<f64>() {
                    if let Some(paren_start) = rest.find('(') {
                        if let Some(paren_end) = rest[paren_start..].find(')') {
                            let inside = &rest[paren_start + 1..paren_start + paren_end];
                            if let Some(slash_pos) = inside.find('/') {
                                let done_str = &inside[..slash_pos];
                                let total_str = &inside[slash_pos + 1..];
                                if let (Ok(done), Ok(total)) =
                                    (done_str.parse::<u64>(), total_str.parse::<u64>())
                                {
                                    return Some((label.to_string(), total, done));
                                }
                            }
                        }
                    }

                    let done = ((pct / 100.0) * 1000.0) as u64;
                    return Some((label.to_string(), 1000, done));
                }
            }
        }
    }

    None
}

fn dir_from_url(url: &str) -> String {
    let url = url.trim_end_matches('/');
    let url = url.strip_suffix(".git").unwrap_or(url);
    url.rsplit(|c| c == '/' || c == ':')
        .next()
        .unwrap_or(url)
        .to_string()
}

async fn open_project(project_dir: &str, config: &mut Config) -> Result<()> {
    let ctx = super::shared::ProjectContext::detect(project_dir)?;
    println!("Found project: {} ({})", ctx.project_name, ctx.project_path);

    let gdio_file = ctx.cwd.join(".gdio");
    if gdio_file.exists() {
        crate::commands::addons::sync::run_sync(config, &ctx.cwd).await?;
    }

    if let Some(editor) = ctx.bound_editor(config)
        && editor.path.exists()
    {
        println!("Opening with {}...", editor.name);
        let editor_path = editor.path.clone();
        let editor_version = editor.version.clone();
        godot::open_project_editor_mode(&editor_path, &ctx.project_file)?;
        super::shared::register_opened_project(config, ctx.cwd, ctx.project_name, &editor_version)?;
        return Ok(());
    }

    if let Some((version, editor)) = ctx.find_editor_for_detected_version(config)
        && editor.path.exists()
    {
        println!("Project requires Godot {}", version);
        println!("Found editor: {}", editor.name);
        let editor_path = editor.path.clone();
        let editor_version = editor.version.clone();
        godot::open_project_editor_mode(&editor_path, &ctx.project_file)?;
        super::shared::register_opened_project(config, ctx.cwd, ctx.project_name, &editor_version)?;
        return Ok(());
    }

    let godot_version = crate::project::parse_godot_version(&ctx.project_file);
    if let Some(ref version) = godot_version {
        println!("Project requires Godot {}", version);
        println!("Godot {} editor not found. Downloading...", version);
        crate::commands::add::download_version_auto(version, false, config).await?;

        if let Some(editor) = config.find_editor_for_version(version) {
            let editor_path = editor.path.clone();
            let editor_version = editor.version.clone();
            godot::open_project_editor_mode(&editor_path, &ctx.project_file)?;
            super::shared::register_opened_project(
                config,
                ctx.cwd,
                ctx.project_name,
                &editor_version,
            )?;
        }
    } else {
        println!("Could not determine Godot version from project.godot");
        println!("Use `gdio --help` for usage information.");
    }

    Ok(())
}
