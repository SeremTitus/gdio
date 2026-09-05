use anyhow::{Context, Result};
use std::process::Command;

pub fn run(_csharp: bool) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let godot_version = super::detect_godot_version(&cwd)?;
    println!(
        "Installing build dependencies for Godot {}...",
        godot_version
    );

    let scripts_dir = cwd.join("misc").join("scripts");
    if !scripts_dir.exists() {
        anyhow::bail!(
            "Godot scripts directory not found at {}",
            scripts_dir.display()
        );
    }

    let scripts: Vec<_> = std::fs::read_dir(&scripts_dir)
        .context("Failed to read scripts directory")?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|s| s.starts_with("install_") && s.ends_with(".py"))
                .unwrap_or(false)
        })
        .collect();

    if scripts.is_empty() {
        println!("No install scripts found.");
        return Ok(());
    }

    let python = if cfg!(target_os = "windows") {
        "python"
    } else {
        "python3"
    };

    for script in &scripts {
        let name = script.file_name();
        let name = name.to_string_lossy();
        println!("  {}...", name);

        match Command::new(python)
            .arg(script.path())
            .current_dir(&cwd)
            .status()
        {
            Ok(s) if s.success() => println!("  ✓ {}", name),
            Ok(s) => println!("  ⚠ {} failed (exit code: {:?})", name, s.code()),
            Err(e) => println!("  ⚠ {} failed: {}", name, e),
        }
    }

    println!("\nDone.");
    Ok(())
}
