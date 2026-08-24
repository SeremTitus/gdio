use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::Config;

const UNINSTALL_ART: &str = include_str!("../../gdio_ascii_art.txt");

pub fn run(keep: bool) -> Result<()> {
    let config_dir = Config::config_dir();
    let install_dir = get_install_dir();

    println!("Uninstalling gdio...");

    // Remove config directory
    if config_dir.exists() {
        if keep {
            println!("Kept config: {}", config_dir.display());
        } else {
            fs::remove_dir_all(&config_dir)?;
            println!("Removed config: {}", config_dir.display());
        }
    }

    // Remove install directory
    if install_dir.exists() {
        schedule_cleanup(&install_dir)?;
    }

    // Remove shell completions
    remove_completions();

    println!();
    print!("{UNINSTALL_ART}");
    println!();
    println!("\x1b[32mThank you for using gdio! :)\x1b[0m");
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
            exe_pid,
            install_dir.display()
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

fn remove_completions() {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return,
    };

    let mut removed = 0;

    // Bash completions — check all possible paths
    let mut bash_paths = vec![
        home.join(".local/share/bash-completion/completions/gdio"),
        home.join(".bash_completion.d/gdio"),
    ];
    if cfg!(target_os = "macos") {
        if let Ok(output) = std::process::Command::new("brew").arg("--prefix").output() {
            if output.status.success() {
                let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
                bash_paths.push(PathBuf::from(format!(
                    "{prefix}/etc/bash_completion.d/gdio"
                )));
            }
        }
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        bash_paths.push(PathBuf::from(format!(
            "{xdg}/bash-completion/completions/gdio"
        )));
    }
    for path in &bash_paths {
        if path.exists() && fs::remove_file(path).is_ok() {
            removed += 1;
        }
    }

    // Zsh completions — check ZDOTDIR and default
    let mut zsh_paths = vec![home.join(".zsh/completions/_gdio")];
    if let Ok(zdotdir) = std::env::var("ZDOTDIR") {
        zsh_paths.push(PathBuf::from(format!("{zdotdir}/.zsh/completions/_gdio")));
    }
    for path in &zsh_paths {
        if path.exists() && fs::remove_file(path).is_ok() {
            removed += 1;
        }
    }

    // Fish completions
    let fish_path = if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(format!("{xdg}/fish/completions/gdio.fish"))
    } else {
        home.join(".config/fish/completions/gdio.fish")
    };
    if fish_path.exists() && fs::remove_file(&fish_path).is_ok() {
        removed += 1;
    }

    // PowerShell completions
    let ps_path = if cfg!(windows) {
        home.join("Documents/PowerShell/Modules/gdio/gdio.psm1")
    } else if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(format!("{xdg}/powershell/Modules/gdio/gdio.psm1"))
    } else {
        home.join(".config/powershell/Modules/gdio/gdio.psm1")
    };
    if ps_path.exists() && fs::remove_file(&ps_path).is_ok() {
        removed += 1;
    }

    if removed > 0 {
        println!("Removed {removed} shell completion file(s)");
    }
}
