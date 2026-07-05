use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;

pub fn run(keep: bool) -> Result<()> {
    let config_dir = Config::config_dir();
    let install_dir = get_install_dir();

    println!("Uninstalling gdio...");

    if config_dir.exists() {
        if keep {
            println!("Kept config: {}", config_dir.display());
        } else {
            fs::remove_dir_all(&config_dir)?;
            println!("Removed config: {}", config_dir.display());
        }
    }

    if install_dir.exists() {
        schedule_cleanup(&install_dir)?;
    }

    println!("\nThank you! for using me :)");
    Ok(())
}

fn schedule_cleanup(install_dir: &Path) -> Result<()> {
    use std::process::Command;

    let exe_pid = std::process::id();

    if cfg!(target_os = "windows") {
        let ps_script = format!(
            "while (Get-Process -Id {} -ErrorAction SilentlyContinue) {{ Start-Sleep -Seconds 1 }}; \
             Remove-Item -Path '{}' -Recurse -Force -ErrorAction SilentlyContinue",
            exe_pid,
            install_dir.display().to_string().replace('\'', "''")
        );

        Command::new("powershell")
            .args(["-WindowStyle", "Hidden", "-Command", &ps_script])
            .spawn()?;
    } else {
        use std::env;

        let script_path = env::temp_dir().join("gdio_cleanup.sh");
        let script_content = format!(
            "#!/bin/sh\n\
             while kill -0 {} 2>/dev/null; do\n\
             \tsleep 1\n\
             done\n\
             rm -rf \"{}\"\n",
            exe_pid, install_dir.display()
        );

        fs::write(&script_path, &script_content)?;

        Command::new("sh")
            .args([script_path.to_string_lossy().as_ref()])
            .spawn()?;
    }

    println!("Scheduled cleanup of: {}", install_dir.display());
    Ok(())
}

fn get_install_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("GDIO_INSTALL_DIR") {
        return PathBuf::from(dir);
    }
    if cfg!(target_os = "windows") {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("gdio")
            .join("bin")
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".gdio")
            .join("bin")
    }
}