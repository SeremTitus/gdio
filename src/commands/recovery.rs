use crate::config::Config;
use crate::gdre;
use anyhow::{Context, Result};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::Command;

fn find_recoverable_files(dir: &Path) -> Vec<PathBuf> {
    let mut pck = Vec::new();
    let mut apk = Vec::new();
    let mut exe = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                match ext.as_str() {
                    "pck" => pck.push(path),
                    "apk" => apk.push(path),
                    "exe" => {
                        let name = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_lowercase();
                        if name.contains("godot") || name.contains("game") {
                            exe.push(path);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    pck.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    apk.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    exe.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    // pck > apk > exe
    if !pck.is_empty() {
        return pck;
    }
    if !apk.is_empty() {
        return apk;
    }
    exe
}

pub async fn run(output: Option<&str>, config: &mut Config) -> Result<()> {
    let gdre_tools_path = gdre::ensure_gdre_tools(config).await?;

    let cwd = std::env::current_dir().context("Failed to get current directory")?;

    let recover_files = find_recoverable_files(&cwd);

    if recover_files.is_empty() {
        anyhow::bail!(
            "No recoverable files found in current directory.\n\
             Expected: .pck, .apk, or game .exe files."
        );
    }

    let recover_file = if recover_files.len() == 1 {
        recover_files[0].clone()
    } else {
        let names: Vec<String> = recover_files
            .iter()
            .map(|p| {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        if std::io::stdin().is_terminal() {
            let idx = dialoguer::FuzzySelect::new()
                .with_prompt("Multiple recoverable files found. Select one")
                .items(&names)
                .default(0)
                .interact()?;
            recover_files[idx].clone()
        } else {
            recover_files.into_iter().next().unwrap()
        }
    };

    let output_path = match output {
        Some(o) => PathBuf::from(o),
        None => cwd.join("recovered"),
    };

    if !output_path.exists() {
        std::fs::create_dir_all(&output_path).with_context(|| {
            format!(
                "Failed to create output directory: {}",
                output_path.display()
            )
        })?;
    }

    println!("Running GDRE Tools recovery...\n");
    println!("  Tool:    {}", gdre_tools_path.display());
    println!("  File:    {}", recover_file.display());
    println!("  Output:  {}\n", output_path.display());

    let status = Command::new(&gdre_tools_path)
        .arg("--headless")
        .arg(format!("--recover={}", recover_file.display()))
        .arg(format!("--output={}", output_path.display()))
        .status()
        .with_context(|| {
            format!(
                "Failed to execute GDRE Tools at '{}'",
                gdre_tools_path.display()
            )
        })?;

    if status.success() {
        println!(
            "Recovery completed successfully.\nOutput: {}",
            output_path.display()
        );
    } else {
        anyhow::bail!(
            "GDRE Tools exited with status: {}",
            status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        );
    }

    Ok(())
}
