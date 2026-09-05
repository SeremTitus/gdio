use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::io::Read;
use std::process::{Command, Stdio};

pub async fn run(tag: Option<&str>, full: bool) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let dir = "godot";
    let target = cwd.join(dir);

    if target.exists() {
        anyhow::bail!(
            "Directory '{}' already exists. Choose a different directory or remove it first.",
            dir
        );
    }

    let url = "https://github.com/godotengine/godot.git";

    if let Some(tag) = tag {
        println!("Cloning Godot {} into {}...", tag, dir);
    } else {
        println!("Cloning Godot (master) into {}...", dir);
    }

    let mut args = vec![
        "clone".to_string(),
        "--progress".to_string(),
        url.to_string(),
        dir.to_string(),
    ];

    if !full {
        args.insert(1, "--depth".to_string());
        args.insert(2, "1".to_string());
    }

    if let Some(tag) = tag {
        args.push("--branch".to_string());
        args.push(tag.to_string());
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
                                    pb.set_message(phase.to_string());
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

    // Verify and print version
    match super::detect_godot_version(&target) {
        Ok(version) => println!("Godot {} cloned successfully to {}", version, dir),
        Err(_) => println!("Godot cloned successfully to {}", dir),
    }

    Ok(())
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
                    if let Some(paren_start) = rest.find('(')
                        && let Some(paren_end) = rest[paren_start..].find(')')
                    {
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

                    let done = ((pct / 100.0) * 1000.0) as u64;
                    return Some((label.to_string(), 1000, done));
                }
            }
        }
    }

    None
}
