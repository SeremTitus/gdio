use crate::config::{self, Config};
use crate::platform::PlatformFlags;
use anyhow::Result;

pub fn run(version: &str, platform: &PlatformFlags, _config: &mut Config) -> Result<()> {
    let (base_version, flavor) = config::parse_version_flavor(version);
    let godot_dir = Config::get_godot_templates_dir().join(format!("{}.{}", base_version, flavor));

    if !godot_dir.exists() {
        anyhow::bail!("No templates found for version: {}", version);
    }

    if !platform.any() {
        std::fs::remove_dir_all(&godot_dir)?;
        println!("Removed all templates for Godot {}", version);
    } else {
        let platforms_to_remove = platform.to_platforms();

        for entry in std::fs::read_dir(&godot_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();

            let should_remove = platforms_to_remove.iter().any(|p| match p.as_str() {
                "windows" => name.starts_with("windows_"),
                "linux" => name.starts_with("linux_"),
                "macos" => name.starts_with("macos"),
                "web" => name.starts_with("web_"),
                "ios" => name.starts_with("ios"),
                "visionos" => name.starts_with("visionos"),
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

        println!(
            "Removed {} templates for Godot {}",
            platforms_to_remove.join(", "),
            version
        );
    }

    Ok(())
}
